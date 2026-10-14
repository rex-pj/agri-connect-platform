// std
use std::{collections::HashMap, sync::Arc};

// external
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    sea_query::{
        extension::postgres::PgExpr, Alias, Expr, Func, Order, Query, SimpleExpr, SubQueryStatement,
    },
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult,
    JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait, RelationTrait, Set,
    Value,
};
use tracing::error;

use domain::{
    models::business::{
        BusinessAdminDetail, BusinessAdminView, BusinessBrowse, BusinessDetail, BusinessGeoResult,
        BusinessProfile, ProductAdminItem, Service, ServiceAdminItem,
    },
    repositories::business_repository_trait::BusinessRepositoryTrait,
};

use domain::Error;

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

    async fn search_geo(
        &self,
        point_wkt: &str,
        radius: f64,
        service_id: Option<i32>,
    ) -> Result<Vec<BusinessGeoResult>, Error> {
        let db = self.db.as_ref();

        // EXISTS subquery: business must have at least one active service
        let mut svc_subq = ServiceEntity::find()
            .select_only()
            .column(service::Column::Id)
            .filter(
                Expr::col((service::Entity, service::Column::BusinessId))
                    .equals((business::Entity, business::Column::Id)),
            )
            .filter(service::Column::IsActive.eq(true))
            .filter(service::Column::DeletedAt.is_null());

        if let Some(svc_id) = service_id {
            svc_subq = svc_subq.filter(service::Column::CategoryId.eq(svc_id));
        }

        let distance_expr = Expr::cust_with_values(
            r#"ST_Distance("businesses".location, ST_GeographyFromText(?))"#,
            [Value::from(point_wkt.to_owned())],
        );
        let within_filter = Expr::cust_with_values(
            r#"ST_DWithin("businesses".location, ST_GeographyFromText(?), ?)"#,
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
            .filter(Expr::exists(svc_subq.into_query()))
            .limit(50);

        QueryTrait::query(&mut query).order_by_expr(Expr::cust("distance_m"), Order::Asc);

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

    // ── Browse — businesses with services (two-query, avoids ARRAY_AGG) ────────

    async fn browse(
        &self,
        service_id: Option<i32>,
        name: Option<String>,
        sort: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<BusinessBrowse>, Error> {
        let db = self.db.as_ref();

        // Build EXISTS subquery: business must have at least one matching active service
        let mut svc_subq = ServiceEntity::find()
            .select_only()
            .column(service::Column::Id)
            .filter(
                Expr::col((service::Entity, service::Column::BusinessId))
                    .equals((business::Entity, business::Column::Id)),
            )
            .filter(service::Column::IsActive.eq(true))
            .filter(service::Column::DeletedAt.is_null());

        if let Some(id) = service_id {
            svc_subq = svc_subq.filter(service::Column::CategoryId.eq(id));
        }

        let mut query = Business::find()
            .select_only()
            .column(business::Column::Id)
            .column(business::Column::Name)
            .column(business::Column::Phone)
            .column(business::Column::Address)
            .column(business::Column::IsVerified)
            .column(business::Column::CreatedOn)
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .filter(Expr::exists(svc_subq.into_query()));

        if let Some(ref q) = name {
            let pattern = format!("%{}%", q);
            query = query.filter(
                Condition::any()
                    .add(Expr::col(business::Column::Name).ilike(pattern.clone()))
                    .add(Expr::col(business::Column::Description).ilike(pattern)),
            );
        }

        let query = match sort.as_deref() {
            Some("newest") => query.order_by_desc(business::Column::CreatedOn),
            Some("name_asc") => query.order_by_asc(business::Column::Name),
            _ => query
                .order_by_desc(business::Column::IsVerified)
                .order_by_desc(business::Column::CreatedOn),
        };

        let businesses = query
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<BusinessBrowseRow>()
            .all(db)
            .await
            .map_err(|e| {
                error!("business browse error: {e}");
                Error::internal("Failed to load services.")
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

    async fn count_browse(
        &self,
        service_id: Option<i32>,
        name: Option<String>,
    ) -> Result<i64, Error> {
        let mut svc_subq = ServiceEntity::find()
            .select_only()
            .column(service::Column::Id)
            .filter(
                Expr::col((service::Entity, service::Column::BusinessId))
                    .equals((business::Entity, business::Column::Id)),
            )
            .filter(service::Column::IsActive.eq(true))
            .filter(service::Column::DeletedAt.is_null());

        if let Some(id) = service_id {
            svc_subq = svc_subq.filter(service::Column::CategoryId.eq(id));
        }

        let mut query = Business::find()
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .filter(Expr::exists(svc_subq.into_query()));

        if let Some(ref q) = name {
            let pattern = format!("%{}%", q);
            query = query.filter(
                Condition::any()
                    .add(Expr::col(business::Column::Name).ilike(pattern.clone()))
                    .add(Expr::col(business::Column::Description).ilike(pattern)),
            );
        }

        query
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("business count_browse error: {e}");
                Error::internal("Failed to load services.")
            })
    }

    async fn featured(&self, limit: i64) -> Result<Vec<BusinessBrowse>, Error> {
        let db = self.db.as_ref();

        let svc_subq = ServiceEntity::find()
            .select_only()
            .column(service::Column::Id)
            .filter(
                Expr::col((service::Entity, service::Column::BusinessId))
                    .equals((business::Entity, business::Column::Id)),
            )
            .filter(service::Column::IsActive.eq(true))
            .filter(service::Column::DeletedAt.is_null())
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
            .map_err(|e| {
                error!("business find_detail services error: {e}");
                Error::internal("Failed to load services.")
            })?;

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
                })
                .collect(),
        }))
    }

    // ── Existence checks ──────────────────────────────────────────────────────

    async fn check_exists(&self, id: i32) -> Result<bool, Error> {
        Business::find()
            .filter(business::Column::Id.eq(id))
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c > 0)
            .map_err(|e| {
                error!("business check_exists error: {e}");
                Error::internal("Database error.")
            })
    }

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
            .map_err(|e| {
                error!("business find_admin_detail services error: {e}");
                Error::internal("Failed to load services.")
            })?;

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

    async fn toggle_verify(&self, id: i32) -> Result<Option<(i32, String, bool)>, Error> {
        let db = self.db.as_ref();

        let b = Business::find()
            .filter(business::Column::Id.eq(id))
            .filter(business::Column::DeletedAt.is_null())
            .one(db)
            .await
            .map_err(|e| {
                error!("business toggle_verify find error (id={id}): {e}");
                Error::internal("Failed to update verification status.")
            })?;

        let Some(b) = b else { return Ok(None) };

        let name = b.name.clone();
        let new_verified = !b.is_verified;
        let mut active: business::ActiveModel = b.into();
        active.is_verified = Set(new_verified);
        active.updated_on = Set(Utc::now().fixed_offset());

        active.update(db).await.map_err(|e| {
            error!("business toggle_verify update error (id={id}): {e}");
            Error::internal("Failed to update verification status.")
        })?;

        Ok(Some((id, name, new_verified)))
    }

    // ── Assign user ───────────────────────────────────────────────────────────

    async fn assign_user(&self, business_id: i32, user_id: i32) -> Result<(), Error> {
        Business::update_many()
            .col_expr(business::Column::OwnerId, Expr::value(user_id))
            .filter(business::Column::Id.eq(business_id))
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("business assign_user error: {e}");
                Error::internal("Failed to link account to business.")
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

    async fn update_profile_text(
        &self,
        user_id: i32,
        name: &str,
        phone: &str,
        description: Option<&str>,
        address: Option<&str>,
    ) -> Result<bool, Error> {
        Business::update_many()
            .col_expr(business::Column::Name, Expr::value(name.to_string()))
            .col_expr(business::Column::Phone, Expr::value(phone.to_string()))
            .col_expr(
                business::Column::Description,
                Expr::value(description.map(str::to_string)),
            )
            .col_expr(
                business::Column::Address,
                Expr::value(address.map(str::to_string)),
            )
            .col_expr(
                business::Column::UpdatedOn,
                Expr::current_timestamp().into(),
            )
            .filter(business::Column::OwnerId.eq(user_id))
            .filter(business::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!("business update_profile_text error (user_id={user_id}): {e}");
                Error::internal("Failed to update profile.")
            })
    }

    // ── Location update (PostGIS ST_GeographyFromText via Expr::cust_with_values) ─

    async fn update_location(&self, user_id: i32, point_wkt: &str) -> Result<(), Error> {
        let mut update = Business::update_many()
            .col_expr(
                business::Column::UpdatedOn,
                Expr::current_timestamp().into(),
            )
            .filter(business::Column::OwnerId.eq(user_id))
            .filter(business::Column::DeletedAt.is_null());

        QueryTrait::query(&mut update).value(
            Alias::new("location"),
            Expr::cust_with_values("ST_GeographyFromText(?)", [point_wkt.to_owned()]),
        );

        update
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("business update_location error (user_id={user_id}): {e}");
                Error::internal("Failed to update GPS location.")
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

    async fn create_for_registration(
        &self,
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
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| {
                error!("business create_for_registration error: {e}");
                Error::internal("Failed to create business profile.")
            })
    }
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
        .map_err(|e| {
            error!("load_service_names error: {e}");
            Error::internal("Failed to load services.")
        })?;

    let mut map: HashMap<i32, Vec<String>> = HashMap::new();
    for r in rows {
        map.entry(r.business_id).or_default().push(r.svc_name);
    }
    Ok(map)
}
