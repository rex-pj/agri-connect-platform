// std
use std::{collections::HashMap, sync::Arc};

// external
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    sea_query::{Alias, Expr, Func, Order, Query, SimpleExpr, SubQueryStatement},
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
    RelationTrait, Set, Value,
};
use tracing::error;

use domain::{
    models::business::{
        BusinessAdminDetail, BusinessAdminView, BusinessBrowse, BusinessDetail, BusinessGeoResult,
        BusinessProfile, ProductAdminItem, ProfileUpdate, ProviderSort, Service, ServiceAdminItem,
    },
    repositories::business_repository_trait::BusinessRepositoryTrait,
};

use domain::{Error, ErrorCode};

use crate::repositories::unique_or_internal;
use crate::search::{text_contains, BUSINESS_SEARCH_COLUMNS};
use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        business::{self, Entity as Business},
        category,
        product::{self, Entity as ProductEntity},
        service::{self, Entity as ServiceEntity},
        user,
    },
};

pub struct BusinessRepository {
    db: Arc<DatabaseConnection>,
}

impl BusinessRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

// ── Row structs for JOIN queries ─────────────────────────────────────────────

#[derive(FromQueryResult)]
struct GeoRow {
    id: i32,
    name: String,
    phone: String,
    address: Option<String>,
    is_verified: bool,
    distance_m: f64,
}

#[derive(FromQueryResult)]
struct BusinessProfileRow {
    id: i32,
    name: String,
    phone: String,
    email: Option<String>,
    description: Option<String>,
    address: Option<String>,
    is_verified: bool,
    lat: Option<f64>,
    lng: Option<f64>,
}

#[derive(FromQueryResult)]
struct BusinessBrowseRow {
    id: i32,
    name: String,
    phone: String,
    address: Option<String>,
    is_verified: bool,
    created_on: DateTimeWithTimeZone,
}

#[derive(FromQueryResult)]
struct SvcNameRow {
    business_id: i32,
    svc_name: String,
}

#[derive(FromQueryResult)]
struct ServiceDetailRow {
    id: i32,
    name: String,
    description: Option<String>,
    price_note: Option<String>,
    category: String,
    category_icon: Option<String>,
}

#[derive(FromQueryResult)]
struct BusinessAdminRow {
    id: i32,
    name: String,
    phone: String,
    address: Option<String>,
    is_verified: bool,
    is_active: bool,
    created_on: DateTimeWithTimeZone,
    updated_on: DateTimeWithTimeZone,
    linked_username: Option<String>,
    created_by_name: Option<String>,
    updated_by_name: Option<String>,
}

#[derive(FromQueryResult)]
struct BizCount {
    business_id: i32,
    count: i64,
}

#[derive(FromQueryResult)]
struct BusinessAdminDetailRow {
    id: i32,
    name: String,
    phone: String,
    email: Option<String>,
    description: Option<String>,
    address: Option<String>,
    is_verified: bool,
    is_active: bool,
    created_on: DateTimeWithTimeZone,
    linked_username: Option<String>,
}

#[derive(FromQueryResult)]
struct ServiceAdminRow {
    id: i32,
    name: String,
    category: String,
    price_note: Option<String>,
    is_active: bool,
}

#[derive(FromQueryResult)]
struct ProductAdminRow {
    id: i32,
    name: String,
    price: Option<i64>,
    unit: Option<String>,
    category: String,
    is_active: bool,
}

#[async_trait]
impl BusinessRepositoryTrait for BusinessRepository {
    // ── Geo search: PostGIS expressions embedded via Expr::cust_with_values ─────
    // Placeholders are `$1`, `$2`: sea-query leaves a `?` in custom SQL unbound
    // for Postgres, and the query fails (convention test in tests/conventions.rs).

    async fn search_geo(
        &self,
        point_wkt: &str,
        radius: f64,
        service_id: Option<i32>,
    ) -> Result<Vec<BusinessGeoResult>, Error> {
        let db = self.db.as_ref();

        let distance_expr = Expr::cust_with_values(
            r#"ST_Distance("businesses".location, ST_GeographyFromText($1))"#,
            [Value::from(point_wkt.to_owned())],
        );
        let within_filter = Expr::cust_with_values(
            r#"ST_DWithin("businesses".location, ST_GeographyFromText($1), $2)"#,
            [Value::from(point_wkt.to_owned()), Value::from(radius)],
        );

        let mut query = Business::find()
            .select_only()
            .column(business::Column::Id)
            .column(business::Column::Name)
            .column(business::Column::Phone)
            .column(business::Column::Address)
            .column(business::Column::IsVerified)
            .expr_as(distance_expr, "distance_m")
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .filter(within_filter)
            .limit(50);
        if let Some(id) = service_id {
            query = query.filter(offers_service(id));
        }

        QueryTrait::query(&mut query).order_by_expr(Expr::cust("distance_m"), Order::Asc);
        let query = query.order_by_asc(business::Column::Id);

        let businesses = query.into_model::<GeoRow>().all(db).await.map_err(|e| {
            error!("business search_geo error: {e}");
            Error::internal("Failed to search services.")
        })?;

        if businesses.is_empty() {
            return Ok(vec![]);
        }

        let ids: Vec<i32> = businesses.iter().map(|b| b.id).collect();
        let svc_map = load_service_names(db, &ids).await?;

        Ok(businesses
            .into_iter()
            .map(|b| {
                let services = svc_map.get(&b.id).cloned().unwrap_or_default();
                BusinessGeoResult {
                    id: b.id,
                    name: b.name,
                    phone: b.phone,
                    address: b.address,
                    is_verified: b.is_verified,
                    distance_m: b.distance_m,
                    services,
                }
            })
            .collect())
    }

    // ── Browse — businesses with their services (two-query, avoids ARRAY_AGG) ─

    async fn browse(
        &self,
        service_id: Option<i32>,
        name: Option<String>,
        sort: ProviderSort,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<BusinessBrowse>, Error> {
        let db = self.db.as_ref();

        let mut query = Business::find()
            .select_only()
            .column(business::Column::Id)
            .column(business::Column::Name)
            .column(business::Column::Phone)
            .column(business::Column::Address)
            .column(business::Column::IsVerified)
            .column(business::Column::CreatedOn)
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null());

        if let Some(id) = service_id {
            query = query.filter(offers_service(id));
        }
        if let Some(q) = name.as_deref().filter(|q| !q.trim().is_empty()) {
            query = query.filter(text_contains(BUSINESS_SEARCH_COLUMNS, q));
        }

        let query = match sort {
            ProviderSort::Newest => query.order_by_desc(business::Column::CreatedOn),
            ProviderSort::NameAsc => query.order_by_asc(business::Column::Name),
            ProviderSort::VerifiedFirst => query
                .order_by_desc(business::Column::IsVerified)
                .order_by_desc(business::Column::CreatedOn),
        };

        let businesses = query
            // A fixed order for ties, so no row repeats or goes missing across pages.
            .order_by_desc(business::Column::Id)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<BusinessBrowseRow>()
            .all(db)
            .await
            .map_err(|e| Error::internal(format!("business browse: {e}")))?;

        if businesses.is_empty() {
            return Ok(vec![]);
        }

        let ids: Vec<i32> = businesses.iter().map(|b| b.id).collect();
        let svc_map = load_service_names(db, &ids).await?;

        Ok(businesses
            .into_iter()
            .map(|b| {
                let services = svc_map.get(&b.id).cloned().unwrap_or_default();
                BusinessBrowse {
                    id: b.id,
                    name: b.name,
                    phone: b.phone,
                    address: b.address,
                    is_verified: b.is_verified,
                    services,
                    created_on: b.created_on.with_timezone(&Utc),
                }
            })
            .collect())
    }

    async fn count_browse(
        &self,
        service_id: Option<i32>,
        name: Option<String>,
    ) -> Result<i64, Error> {
        // The same conditions as `browse`, so the page count matches the pages.
        let mut query = Business::find()
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null());

        if let Some(id) = service_id {
            query = query.filter(offers_service(id));
        }
        if let Some(q) = name.as_deref().filter(|q| !q.trim().is_empty()) {
            query = query.filter(text_contains(BUSINESS_SEARCH_COLUMNS, q));
        }

        query
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| Error::internal(format!("business count_browse: {e}")))
    }

    async fn featured(&self, limit: i64) -> Result<Vec<BusinessBrowse>, Error> {
        let db = self.db.as_ref();

        // The home page shows off providers with something to offer.
        let svc_subq = ServiceEntity::find()
            .select_only()
            .column(service::Column::Id)
            .join(JoinType::InnerJoin, service::Relation::Category.def())
            .filter(
                Expr::col((service::Entity, service::Column::BusinessId))
                    .equals((business::Entity, business::Column::Id)),
            )
            .filter(service::Column::IsActive.eq(true))
            .filter(service::Column::DeletedAt.is_null())
            .filter(category::Column::IsActive.eq(true))
            .into_query();

        let businesses = Business::find()
            .select_only()
            .column(business::Column::Id)
            .column(business::Column::Name)
            .column(business::Column::Phone)
            .column(business::Column::Address)
            .column(business::Column::IsVerified)
            .column(business::Column::CreatedOn)
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .filter(Expr::exists(svc_subq))
            .order_by_desc(business::Column::IsVerified)
            .order_by_desc(business::Column::CreatedOn)
            .order_by_desc(business::Column::Id)
            .limit(limit as u64)
            .into_model::<BusinessBrowseRow>()
            .all(db)
            .await
            .map_err(|e| {
                error!("business featured error: {e}");
                Error::internal("Failed to load providers.")
            })?;

        if businesses.is_empty() {
            return Ok(vec![]);
        }

        let ids: Vec<i32> = businesses.iter().map(|b| b.id).collect();
        let svc_map = load_service_names(db, &ids).await?;

        Ok(businesses
            .into_iter()
            .map(|b| {
                let services = svc_map.get(&b.id).cloned().unwrap_or_default();
                BusinessBrowse {
                    id: b.id,
                    name: b.name,
                    phone: b.phone,
                    address: b.address,
                    is_verified: b.is_verified,
                    services,
                    created_on: b.created_on.with_timezone(&Utc),
                }
            })
            .collect())
    }

    // ── find_detail ───────────────────────────────────────────────────────────

    async fn find_detail(&self, id: i32) -> Result<Option<BusinessDetail>, Error> {
        let db = self.db.as_ref();

        let b = match Business::find_by_id(id)
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .one(db)
            .await
            .map_err(|e| {
                error!("business find_detail error: {e}");
                Error::internal("Failed to load provider details.")
            })? {
            None => return Ok(None),
            Some(b) => b,
        };

        let svc_rows = ServiceEntity::find()
            .select_only()
            .column(service::Column::Id)
            .column(service::Column::Name)
            .column(service::Column::Description)
            .column(service::Column::PriceNote)
            .column_as(category::Column::Name, "category")
            .column_as(category::Column::Icon, "category_icon")
            .join(JoinType::InnerJoin, service::Relation::Category.def())
            .filter(service::Column::BusinessId.eq(b.id))
            .filter(service::Column::IsActive.eq(true))
            .filter(service::Column::DeletedAt.is_null())
            .filter(category::Column::IsActive.eq(true))
            .order_by_asc(category::Column::DisplayOrder)
            .order_by_desc(service::Column::CreatedOn)
            .into_model::<ServiceDetailRow>()
            .all(db)
            .await
            .map_err(|e| Error::internal(format!("business find_detail services: {e}")))?;

        Ok(Some(BusinessDetail {
            id: b.id,
            name: b.name,
            phone: b.phone,
            description: b.description,
            address: b.address,
            is_verified: b.is_verified,
            services: svc_rows
                .into_iter()
                .map(|r| Service {
                    id: r.id,
                    name: r.name,
                    description: r.description,
                    price_note: r.price_note,
                    category: r.category,
                    category_icon: r.category_icon,
                })
                .collect(),
        }))
    }

    // ── Existence checks ──────────────────────────────────────────────────────

    async fn check_exists_for_admin(&self, id: i32) -> Result<bool, Error> {
        Business::find()
            .filter(business::Column::Id.eq(id))
            .filter(business::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c > 0)
            .map_err(|e| {
                error!("business check_exists_for_admin error: {e}");
                Error::internal("Database error.")
            })
    }

    // ── Admin list ────────────────────────────────────────────────────────────

    async fn list_for_admin(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<BusinessAdminView>, Error> {
        let db = self.db.as_ref();

        let businesses = Business::find()
            .select_only()
            .column(business::Column::Id)
            .column(business::Column::Name)
            .column(business::Column::Phone)
            .column(business::Column::Address)
            .column(business::Column::IsVerified)
            .column(business::Column::IsActive)
            .column(business::Column::CreatedOn)
            .column(business::Column::UpdatedOn)
            .column_as(user::Column::Username, "linked_username")
            .expr_as(
                SimpleExpr::SubQuery(
                    None,
                    Box::new(SubQueryStatement::SelectStatement(
                        Query::select()
                            .from(user::Entity)
                            .column(user::Column::Username)
                            .and_where(
                                Expr::col((user::Entity, user::Column::Id))
                                    .equals((business::Entity, business::Column::CreatedBy)),
                            )
                            .to_owned(),
                    )),
                ),
                "created_by_name",
            )
            .expr_as(
                SimpleExpr::SubQuery(
                    None,
                    Box::new(SubQueryStatement::SelectStatement(
                        Query::select()
                            .from(user::Entity)
                            .column(user::Column::Username)
                            .and_where(
                                Expr::col((user::Entity, user::Column::Id))
                                    .equals((business::Entity, business::Column::UpdatedBy)),
                            )
                            .to_owned(),
                    )),
                ),
                "updated_by_name",
            )
            .join(JoinType::LeftJoin, business::Relation::Owner.def())
            .filter(business::Column::DeletedAt.is_null())
            .order_by_desc(business::Column::IsVerified)
            .order_by_desc(business::Column::CreatedOn)
            .order_by_desc(business::Column::Id)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<BusinessAdminRow>()
            .all(db)
            .await
            .map_err(|e| {
                error!("business list_for_admin error: {e}");
                Error::internal("Failed to load providers.")
            })?;

        if businesses.is_empty() {
            return Ok(vec![]);
        }

        let page_ids: Vec<i32> = businesses.iter().map(|b| b.id).collect();

        let (svc_counts, prod_counts) = tokio::try_join!(
            ServiceEntity::find()
                .select_only()
                .column(service::Column::BusinessId)
                .expr_as(Func::count(Expr::col(service::Column::Id)), "count")
                .filter(service::Column::BusinessId.is_in(page_ids.clone()))
                .filter(service::Column::IsActive.eq(true))
                .filter(service::Column::DeletedAt.is_null())
                .group_by(service::Column::BusinessId)
                .into_model::<BizCount>()
                .all(db),
            ProductEntity::find()
                .select_only()
                .column(product::Column::BusinessId)
                .expr_as(Func::count(Expr::col(product::Column::Id)), "count")
                .filter(product::Column::BusinessId.is_in(page_ids))
                .filter(product::Column::IsActive.eq(true))
                .filter(product::Column::DeletedAt.is_null())
                .group_by(product::Column::BusinessId)
                .into_model::<BizCount>()
                .all(db),
        )
        .map_err(|e| {
            error!("business list_for_admin counts error: {e}");
            Error::internal("Failed to load providers.")
        })?;

        let svc_map: HashMap<i32, i64> = svc_counts
            .into_iter()
            .map(|r| (r.business_id, r.count))
            .collect();
        let prod_map: HashMap<i32, i64> = prod_counts
            .into_iter()
            .map(|r| (r.business_id, r.count))
            .collect();

        Ok(businesses
            .into_iter()
            .map(|b| BusinessAdminView {
                id: b.id,
                name: b.name,
                phone: b.phone,
                address: b.address,
                is_verified: b.is_verified,
                is_active: b.is_active,
                service_count: *svc_map.get(&b.id).unwrap_or(&0),
                product_count: *prod_map.get(&b.id).unwrap_or(&0),
                created_on: b.created_on.with_timezone(&Utc),
                updated_on: b.updated_on.with_timezone(&Utc),
                linked_username: b.linked_username,
                created_by_name: b.created_by_name,
                updated_by_name: b.updated_by_name,
            })
            .collect())
    }

    async fn find_admin_detail(&self, id: i32) -> Result<Option<BusinessAdminDetail>, Error> {
        let db = self.db.as_ref();

        let core = match Business::find()
            .select_only()
            .column(business::Column::Id)
            .column(business::Column::Name)
            .column(business::Column::Phone)
            .column(business::Column::Email)
            .column(business::Column::Description)
            .column(business::Column::Address)
            .column(business::Column::IsVerified)
            .column(business::Column::IsActive)
            .column(business::Column::CreatedOn)
            .column_as(user::Column::Username, "linked_username")
            .join(JoinType::LeftJoin, business::Relation::Owner.def())
            .filter(business::Column::Id.eq(id))
            .filter(business::Column::DeletedAt.is_null())
            .into_model::<BusinessAdminDetailRow>()
            .one(db)
            .await
            .map_err(|e| {
                error!("business find_admin_detail core error: {e}");
                Error::internal("Failed to load provider details.")
            })? {
            None => return Ok(None),
            Some(r) => r,
        };

        let svc_rows = ServiceEntity::find()
            .select_only()
            .column(service::Column::Id)
            .column(service::Column::Name)
            .column_as(category::Column::Name, "category")
            .column(service::Column::PriceNote)
            .column(service::Column::IsActive)
            .join(JoinType::InnerJoin, service::Relation::Category.def())
            .filter(service::Column::BusinessId.eq(id))
            .filter(service::Column::DeletedAt.is_null())
            .order_by_asc(category::Column::DisplayOrder)
            .order_by_desc(service::Column::CreatedOn)
            .into_model::<ServiceAdminRow>()
            .all(db)
            .await
            .map_err(|e| Error::internal(format!("business find_admin_detail services: {e}")))?;

        let prod_rows = ProductEntity::find()
            .select_only()
            .column(product::Column::Id)
            .column(product::Column::Name)
            .column(product::Column::Price)
            .column(product::Column::Unit)
            .column_as(category::Column::Name, "category")
            .column(product::Column::IsActive)
            .join(JoinType::InnerJoin, product::Relation::Category.def())
            .filter(product::Column::BusinessId.eq(id))
            .filter(product::Column::DeletedAt.is_null())
            .order_by_asc(category::Column::DisplayOrder)
            .order_by_desc(product::Column::CreatedOn)
            .into_model::<ProductAdminRow>()
            .all(db)
            .await
            .map_err(|e| {
                error!("business find_admin_detail products error: {e}");
                Error::internal("Failed to load products.")
            })?;

        Ok(Some(BusinessAdminDetail {
            id: core.id,
            name: core.name,
            phone: core.phone,
            email: core.email,
            description: core.description,
            address: core.address,
            is_verified: core.is_verified,
            is_active: core.is_active,
            created_on: core.created_on.with_timezone(&Utc),
            linked_username: core.linked_username,
            services: svc_rows
                .into_iter()
                .map(|r| ServiceAdminItem {
                    id: r.id,
                    name: r.name,
                    category: r.category,
                    price_note: r.price_note,
                    is_active: r.is_active,
                })
                .collect(),
            products: prod_rows
                .into_iter()
                .map(|r| ProductAdminItem {
                    id: r.id,
                    name: r.name,
                    price: r.price,
                    unit: r.unit,
                    category: r.category,
                    is_active: r.is_active,
                })
                .collect(),
        }))
    }

    // ── Toggle verify ─────────────────────────────────────────────────────────

    async fn set_verified(&self, id: i32, verified: bool) -> Result<Option<(String, bool)>, Error> {
        let db = self.db.as_ref();

        let b = Business::find()
            .filter(business::Column::Id.eq(id))
            .filter(business::Column::DeletedAt.is_null())
            .one(db)
            .await
            .map_err(|e| {
                error!("business set_verified find error (id={id}): {e}");
                Error::internal("Failed to update verification status.")
            })?;

        let Some(b) = b else { return Ok(None) };

        let name = b.name.clone();
        if b.is_verified == verified {
            return Ok(Some((name, false)));
        }
        let mut active: business::ActiveModel = b.into();
        active.is_verified = Set(verified);
        active.updated_on = Set(Utc::now().fixed_offset());

        active.update(db).await.map_err(|e| {
            error!("business set_verified update error (id={id}): {e}");
            Error::internal("Failed to update verification status.")
        })?;

        Ok(Some((name, true)))
    }

    // ── Assign user ───────────────────────────────────────────────────────────

    async fn assign_user(&self, business_id: i32, user_id: i32) -> Result<(), Error> {
        Business::update_many()
            .col_expr(business::Column::OwnerId, Expr::value(user_id))
            .filter(business::Column::Id.eq(business_id))
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            // A concurrent assignment gave the account another business first.
            .map_err(|e| {
                unique_or_internal(
                    e,
                    "username",
                    ErrorCode::ProviderAccountInUse,
                    "business assign_user",
                )
            })
    }

    // ── Profile (includes PostGIS ST_Y/ST_X via Expr::cust) ──────────────────

    async fn get_profile_by_user(&self, user_id: i32) -> Result<Option<BusinessProfile>, Error> {
        let row = Business::find()
            .select_only()
            .column(business::Column::Id)
            .column(business::Column::Name)
            .column(business::Column::Phone)
            .column(business::Column::Email)
            .column(business::Column::Description)
            .column(business::Column::Address)
            .column(business::Column::IsVerified)
            .expr_as(
                Expr::cust(r#"ST_Y("businesses".location::geometry)"#),
                "lat",
            )
            .expr_as(
                Expr::cust(r#"ST_X("businesses".location::geometry)"#),
                "lng",
            )
            .filter(business::Column::OwnerId.eq(user_id))
            .filter(business::Column::DeletedAt.is_null())
            .limit(1)
            .into_model::<BusinessProfileRow>()
            .one(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("business get_profile_by_user error (user_id={user_id}): {e}");
                Error::internal("Failed to load profile.")
            })?;

        Ok(row.map(|r| BusinessProfile {
            id: r.id,
            name: r.name,
            phone: r.phone,
            email: r.email,
            description: r.description,
            address: r.address,
            is_verified: r.is_verified,
            lat: r.lat,
            lng: r.lng,
        }))
    }

    // ── Profile update (location via ST_GeographyFromText in the same UPDATE) ─

    async fn update_profile(&self, user_id: i32, profile: &ProfileUpdate) -> Result<bool, Error> {
        let mut update = Business::update_many()
            .col_expr(business::Column::Name, Expr::value(profile.name.clone()))
            .col_expr(business::Column::Phone, Expr::value(profile.phone.clone()))
            .col_expr(
                business::Column::Description,
                Expr::value(profile.description.clone()),
            )
            .col_expr(
                business::Column::Address,
                Expr::value(profile.address.clone()),
            )
            .col_expr(
                business::Column::UpdatedOn,
                Expr::current_timestamp().into(),
            )
            .filter(business::Column::OwnerId.eq(user_id))
            .filter(business::Column::DeletedAt.is_null());

        // The location column is not in the entity (PostGIS type).
        let location = match &profile.location_wkt {
            Some(wkt) => Expr::cust_with_values("ST_GeographyFromText($1)", [wkt.clone()]),
            None => Expr::cust("NULL"),
        };
        QueryTrait::query(&mut update).value(Alias::new("location"), location);

        update
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!("business update_profile error (user_id={user_id}): {e}");
                Error::internal("Failed to update profile.")
            })
    }

    // ── Entity API helpers ────────────────────────────────────────────────────

    async fn get_business_id_by_user(&self, user_id: i32) -> Result<Option<i32>, Error> {
        Business::find()
            .filter(business::Column::OwnerId.eq(user_id))
            .filter(business::Column::DeletedAt.is_null())
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(|b| b.id))
            .map_err(|e| {
                error!("business get_business_id_by_user error: {e}");
                Error::internal("Failed to resolve business.")
            })
    }

    async fn count_all(&self) -> Result<i64, Error> {
        Business::find()
            .filter(business::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("business count_all error: {e}");
                Error::internal("Failed to load stats.")
            })
    }

    async fn count_verified(&self) -> Result<i64, Error> {
        Business::find()
            .filter(business::Column::IsVerified.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("business count_verified error: {e}");
                Error::internal("Failed to load statistics.")
            })
    }
}

/// The business a provider signs up with, owned by `user_id`; inside the
/// sign-up's transaction (see `UserRepository::create_with_business`).
pub(crate) async fn insert_registration_business<C: ConnectionTrait>(
    db: &C,
    name: &str,
    phone: &str,
    user_id: i32,
) -> Result<i32, Error> {
    let new_business = business::ActiveModel {
        name: Set(name.to_string()),
        phone: Set(phone.to_string()),
        owner_id: Set(Some(user_id)),
        is_verified: Set(false),
        is_active: Set(true),
        ..Default::default()
    };

    Business::insert(new_business)
        .exec(db)
        .await
        .map(|r| r.last_insert_id)
        .map_err(|e| {
            error!("business create_for_registration error: {e}");
            Error::internal("Failed to create business profile.")
        })
}

// ── Shared helper: the "offers this kind of service" filter ───────────────────

/// The business offers a live service in the live category `category_id`.
/// The only service condition on the public lists: a provider with no
/// services yet (a new sign-up) is still listed, so farmers can find it.
fn offers_service(category_id: i32) -> SimpleExpr {
    Expr::exists(
        ServiceEntity::find()
            .select_only()
            .column(service::Column::Id)
            .join(JoinType::InnerJoin, service::Relation::Category.def())
            .filter(
                Expr::col((service::Entity, service::Column::BusinessId))
                    .equals((business::Entity, business::Column::Id)),
            )
            .filter(service::Column::IsActive.eq(true))
            .filter(service::Column::DeletedAt.is_null())
            .filter(service::Column::CategoryId.eq(category_id))
            .filter(category::Column::IsActive.eq(true))
            .into_query(),
    )
}

// ── Shared helper: load distinct service category names per business ──────────

async fn load_service_names(
    db: &DatabaseConnection,
    ids: &[i32],
) -> Result<HashMap<i32, Vec<String>>, Error> {
    let rows = ServiceEntity::find()
        .select_only()
        .column(service::Column::BusinessId)
        .column_as(category::Column::Name, "svc_name")
        .join(JoinType::InnerJoin, service::Relation::Category.def())
        .filter(service::Column::BusinessId.is_in(ids.to_vec()))
        .filter(service::Column::IsActive.eq(true))
        .filter(service::Column::DeletedAt.is_null())
        .filter(category::Column::IsActive.eq(true))
        .distinct()
        .order_by_asc(category::Column::Name)
        .into_model::<SvcNameRow>()
        .all(db)
        .await
        .map_err(|e| Error::internal(format!("load_service_names: {e}")))?;

    let mut map: HashMap<i32, Vec<String>> = HashMap::new();
    for r in rows {
        map.entry(r.business_id).or_default().push(r.svc_name);
    }
    Ok(map)
}
