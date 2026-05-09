use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend,
    EntityTrait, FromQueryResult, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set, Statement,
};
use sea_orm::sea_query::{Expr, Func};
use sea_orm::QueryTrait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::error;

use crate::{
    infrastructure::db_types::DateTimeWithTimeZone,
    domain::{
        models::business::{
            BusinessAdminDetail, BusinessAdminView, BusinessBrowse, BusinessDetail,
            BusinessGeoResult, BusinessProfile, ProductAdminItem, Service, ServiceAdminItem,
        },
        repositories::business_repository_trait::BusinessRepositoryTrait,
    },
    entities::{
        business::{self, Entity as Business},
        category,
        product::{self, Entity as ProductEntity},
        service::{self, Entity as ServiceEntity},
        user,
    },
    view_models::AppError,
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
struct BusinessBrowseRow {
    id:          i32,
    name:        String,
    phone:       String,
    address:     Option<String>,
    is_verified: bool,
    created_on:  DateTimeWithTimeZone,
}

#[derive(FromQueryResult)]
struct SvcNameRow {
    business_id: i32,
    svc_name:    String,
}

#[derive(FromQueryResult)]
struct ServiceDetailRow {
    id:          i32,
    name:        String,
    description: Option<String>,
    price_note:  Option<String>,
    category:    String,
}

#[derive(FromQueryResult)]
struct BusinessAdminRow {
    id:              i32,
    name:            String,
    phone:           String,
    address:         Option<String>,
    is_verified:     bool,
    is_active:       bool,
    created_on:      DateTimeWithTimeZone,
    linked_username: Option<String>,
}

#[derive(FromQueryResult)]
struct BizCount {
    business_id: i32,
    count:       i64,
}

#[derive(FromQueryResult)]
struct BusinessAdminDetailRow {
    id:              i32,
    name:            String,
    phone:           String,
    email:           Option<String>,
    description:     Option<String>,
    address:         Option<String>,
    is_verified:     bool,
    is_active:       bool,
    created_on:      DateTimeWithTimeZone,
    linked_username: Option<String>,
}

#[derive(FromQueryResult)]
struct ServiceAdminRow {
    id:         i32,
    name:       String,
    category:   String,
    price_note: Option<String>,
    is_active:  bool,
}

#[derive(FromQueryResult)]
struct ProductAdminRow {
    id:        i32,
    name:      String,
    price:     Option<i64>,
    unit:      Option<String>,
    category:  String,
    is_active: bool,
}

#[async_trait]
impl BusinessRepositoryTrait for BusinessRepository {
    // ── PostGIS geo search — must stay as raw SQL (no query builder support) ───

    async fn search_geo(
        &self,
        point_wkt: &str,
        radius: f64,
        service_id: Option<i32>,
    ) -> Result<Vec<BusinessGeoResult>, AppError> {
        let stmt = if let Some(svc_id) = service_id {
            Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    b.id, b.name, b.phone, b.address, b.is_verified,
                    ST_Distance(b.location, ST_GeographyFromText($1)) AS distance_m,
                    ARRAY_AGG(DISTINCT c.name ORDER BY c.name) AS services
                FROM businesses b
                JOIN services sl
                    ON  sl.business_id = b.id AND sl.is_active = TRUE AND sl.deleted_at IS NULL
                JOIN categories c
                    ON  c.id = sl.category_id AND c.is_active = TRUE
                WHERE
                    b.is_active = TRUE AND b.deleted_at IS NULL
                    AND ST_DWithin(b.location, ST_GeographyFromText($2), $3)
                    AND EXISTS (
                        SELECT 1 FROM services sl2
                        WHERE sl2.business_id = b.id AND sl2.category_id = $4
                          AND sl2.is_active = TRUE AND sl2.deleted_at IS NULL
                    )
                GROUP BY b.id, b.name, b.phone, b.address, b.is_verified, b.location
                ORDER BY distance_m ASC
                LIMIT 50
                "#,
                [point_wkt.into(), point_wkt.into(), radius.into(), svc_id.into()],
            )
        } else {
            Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    b.id, b.name, b.phone, b.address, b.is_verified,
                    ST_Distance(b.location, ST_GeographyFromText($1)) AS distance_m,
                    ARRAY_AGG(DISTINCT c.name ORDER BY c.name) AS services
                FROM businesses b
                JOIN services sl
                    ON  sl.business_id = b.id AND sl.is_active = TRUE AND sl.deleted_at IS NULL
                JOIN categories c
                    ON  c.id = sl.category_id AND c.is_active = TRUE
                WHERE b.is_active = TRUE AND b.deleted_at IS NULL
                    AND ST_DWithin(b.location, ST_GeographyFromText($2), $3)
                GROUP BY b.id, b.name, b.phone, b.address, b.is_verified, b.location
                ORDER BY distance_m ASC
                LIMIT 50
                "#,
                [point_wkt.into(), point_wkt.into(), radius.into()],
            )
        };

        let rows = self.db.query_all(stmt).await.map_err(|e| {
            error!("business search_geo error: {e}");
            AppError::Internal("Failed to search services.".into())
        })?;

        Ok(rows
            .into_iter()
            .map(|row| BusinessGeoResult {
                id:          row.try_get("", "id").unwrap_or_default(),
                name:        row.try_get("", "name").unwrap_or_default(),
                phone:       row.try_get("", "phone").unwrap_or_default(),
                address:     row.try_get("", "address").unwrap_or_default(),
                is_verified: row.try_get("", "is_verified").unwrap_or_default(),
                distance_m:  row.try_get("", "distance_m").unwrap_or_default(),
                services:    row.try_get("", "services").unwrap_or_default(),
            })
            .collect())
    }

    // ── Browse — businesses with services (two-query, avoids ARRAY_AGG) ────────

    async fn browse(
        &self,
        service_id: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<BusinessBrowse>, AppError> {
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
            .filter(Expr::exists(svc_subq.into_query()))
            .order_by_desc(business::Column::IsVerified)
            .order_by_desc(business::Column::CreatedOn)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<BusinessBrowseRow>()
            .all(db)
            .await
            .map_err(|e| {
                error!("business browse error: {e}");
                AppError::Internal("Failed to load services.".into())
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
                    id:          b.id,
                    name:        b.name,
                    phone:       b.phone,
                    address:     b.address,
                    is_verified: b.is_verified,
                    services,
                    created_on:  b.created_on.with_timezone(&Utc),
                }
            })
            .collect())
    }

    async fn count_browse(&self, service_id: Option<i32>) -> Result<i64, AppError> {
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

        Business::find()
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .filter(Expr::exists(svc_subq.into_query()))
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("business count_browse error: {e}");
                AppError::Internal("Failed to load services.".into())
            })
    }

    async fn featured(&self, limit: i64) -> Result<Vec<BusinessBrowse>, AppError> {
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
                AppError::Internal("Failed to load providers.".into())
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
                    id:          b.id,
                    name:        b.name,
                    phone:       b.phone,
                    address:     b.address,
                    is_verified: b.is_verified,
                    services,
                    created_on:  b.created_on.with_timezone(&Utc),
                }
            })
            .collect())
    }

    // ── find_detail ───────────────────────────────────────────────────────────

    async fn find_detail(&self, id: i32) -> Result<Option<BusinessDetail>, AppError> {
        let db = self.db.as_ref();

        let b = match Business::find_by_id(id)
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .one(db)
            .await
            .map_err(|e| {
                error!("business find_detail error: {e}");
                AppError::Internal("Failed to load provider details.".into())
            })? {
            None    => return Ok(None),
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
                AppError::Internal("Failed to load services.".into())
            })?;

        Ok(Some(BusinessDetail {
            id:          b.id,
            name:        b.name,
            phone:       b.phone,
            description: b.description,
            address:     b.address,
            is_verified: b.is_verified,
            services:    svc_rows
                .into_iter()
                .map(|r| Service {
                    id:          r.id,
                    name:        r.name,
                    description: r.description,
                    price_note:  r.price_note,
                    category:    r.category,
                })
                .collect(),
        }))
    }

    // ── Existence checks ──────────────────────────────────────────────────────

    async fn check_exists(&self, id: i32) -> Result<bool, AppError> {
        Business::find()
            .filter(business::Column::Id.eq(id))
            .filter(business::Column::IsActive.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c > 0)
            .map_err(|e| {
                error!("business check_exists error: {e}");
                AppError::Internal("Database error.".into())
            })
    }

    async fn check_exists_for_admin(&self, id: i32) -> Result<bool, AppError> {
        Business::find()
            .filter(business::Column::Id.eq(id))
            .filter(business::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c > 0)
            .map_err(|e| {
                error!("business check_exists_for_admin error: {e}");
                AppError::Internal("Database error.".into())
            })
    }

    // ── Admin list ────────────────────────────────────────────────────────────

    async fn list_for_admin(&self, limit: i64, offset: i64) -> Result<Vec<BusinessAdminView>, AppError> {
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
            .column_as(user::Column::Username, "linked_username")
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
                AppError::Internal("Failed to load providers.".into())
            })?;

        if businesses.is_empty() {
            return Ok(vec![]);
        }

        let svc_counts = ServiceEntity::find()
            .select_only()
            .column(service::Column::BusinessId)
            .expr_as(
                Func::count(Expr::col(service::Column::Id)),
                "count",
            )
            .filter(service::Column::IsActive.eq(true))
            .filter(service::Column::DeletedAt.is_null())
            .group_by(service::Column::BusinessId)
            .into_model::<BizCount>()
            .all(db)
            .await
            .map_err(|e| {
                error!("business list_for_admin svc_counts error: {e}");
                AppError::Internal("Failed to load providers.".into())
            })?;

        let prod_counts = ProductEntity::find()
            .select_only()
            .column(product::Column::BusinessId)
            .expr_as(
                Func::count(Expr::col(product::Column::Id)),
                "count",
            )
            .filter(product::Column::IsActive.eq(true))
            .filter(product::Column::DeletedAt.is_null())
            .group_by(product::Column::BusinessId)
            .into_model::<BizCount>()
            .all(db)
            .await
            .map_err(|e| {
                error!("business list_for_admin prod_counts error: {e}");
                AppError::Internal("Failed to load providers.".into())
            })?;

        let svc_map: HashMap<i32, i64> =
            svc_counts.into_iter().map(|r| (r.business_id, r.count)).collect();
        let prod_map: HashMap<i32, i64> =
            prod_counts.into_iter().map(|r| (r.business_id, r.count)).collect();

        Ok(businesses
            .into_iter()
            .map(|b| BusinessAdminView {
                id:              b.id,
                name:            b.name,
                phone:           b.phone,
                address:         b.address,
                is_verified:     b.is_verified,
                is_active:       b.is_active,
                service_count:   *svc_map.get(&b.id).unwrap_or(&0),
                product_count:   *prod_map.get(&b.id).unwrap_or(&0),
                created_on:      b.created_on.with_timezone(&Utc),
                linked_username: b.linked_username,
            })
            .collect())
    }

    async fn find_admin_detail(&self, id: i32) -> Result<Option<BusinessAdminDetail>, AppError> {
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
                AppError::Internal("Failed to load provider details.".into())
            })? {
            None    => return Ok(None),
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
                AppError::Internal("Failed to load services.".into())
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
                AppError::Internal("Failed to load products.".into())
            })?;

        Ok(Some(BusinessAdminDetail {
            id:              core.id,
            name:            core.name,
            phone:           core.phone,
            email:           core.email,
            description:     core.description,
            address:         core.address,
            is_verified:     core.is_verified,
            is_active:       core.is_active,
            created_on:      core.created_on.with_timezone(&Utc),
            linked_username: core.linked_username,
            services:        svc_rows
                .into_iter()
                .map(|r| ServiceAdminItem {
                    id:         r.id,
                    name:       r.name,
                    category:   r.category,
                    price_note: r.price_note,
                    is_active:  r.is_active,
                })
                .collect(),
            products:        prod_rows
                .into_iter()
                .map(|r| ProductAdminItem {
                    id:        r.id,
                    name:      r.name,
                    price:     r.price,
                    unit:      r.unit,
                    category:  r.category,
                    is_active: r.is_active,
                })
                .collect(),
        }))
    }

    // ── Toggle verify ─────────────────────────────────────────────────────────

    async fn toggle_verify(&self, id: i32) -> Result<Option<(i32, String, bool)>, AppError> {
        let db = self.db.as_ref();

        let model = match Business::find_by_id(id)
            .filter(business::Column::DeletedAt.is_null())
            .one(db)
            .await
            .map_err(|e| {
                error!("business toggle_verify find error (id={id}): {e}");
                AppError::Internal("Failed to update verification status.".into())
            })? {
            None    => return Ok(None),
            Some(b) => b,
        };

        let new_verified = !model.is_verified;
        let mut active: business::ActiveModel = model.into();
        active.is_verified = Set(new_verified);

        let updated = active.update(db).await.map_err(|e| {
            error!("business toggle_verify update error (id={id}): {e}");
            AppError::Internal("Failed to update verification status.".into())
        })?;

        Ok(Some((updated.id, updated.name, updated.is_verified)))
    }

    // ── Assign user ───────────────────────────────────────────────────────────

    async fn assign_user(&self, business_id: i32, user_id: i32) -> Result<(), AppError> {
        Business::update_many()
            .col_expr(business::Column::OwnerId, Expr::value(user_id))
            .filter(business::Column::Id.eq(business_id))
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("business assign_user error: {e}");
                AppError::Internal("Failed to link account to business.".into())
            })
    }

    // ── PostGIS profile — must stay as raw SQL ────────────────────────────────

    async fn get_profile_by_user(&self, user_id: i32) -> Result<Option<BusinessProfile>, AppError> {
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                b.id, b.name, b.phone, b.email, b.description, b.address, b.is_verified,
                ST_Y(b.location::geometry) AS lat,
                ST_X(b.location::geometry) AS lng
            FROM businesses b
            WHERE b.owner_id = $1 AND b.deleted_at IS NULL
            LIMIT 1
            "#,
            [user_id.into()],
        );

        let row = self.db.query_one(stmt).await.map_err(|e| {
            error!("business get_profile_by_user error (user_id={user_id}): {e}");
            AppError::Internal("Failed to load profile.".into())
        })?;

        Ok(row.map(|r| BusinessProfile {
            id:          r.try_get("", "id").unwrap_or_default(),
            name:        r.try_get("", "name").unwrap_or_default(),
            phone:       r.try_get("", "phone").unwrap_or_default(),
            email:       r.try_get("", "email").unwrap_or_default(),
            description: r.try_get("", "description").unwrap_or_default(),
            address:     r.try_get("", "address").unwrap_or_default(),
            is_verified: r.try_get("", "is_verified").unwrap_or_default(),
            lat:         r.try_get("", "lat").unwrap_or_default(),
            lng:         r.try_get("", "lng").unwrap_or_default(),
        }))
    }

    async fn update_profile_text(
        &self,
        user_id: i32,
        name: &str,
        phone: &str,
        description: Option<&str>,
        address: Option<&str>,
    ) -> Result<bool, AppError> {
        Business::update_many()
            .col_expr(business::Column::Name,        Expr::value(name.to_string()))
            .col_expr(business::Column::Phone,       Expr::value(phone.to_string()))
            .col_expr(business::Column::Description, Expr::value(description.map(str::to_string)))
            .col_expr(business::Column::Address,     Expr::value(address.map(str::to_string)))
            .col_expr(business::Column::UpdatedOn,   Expr::current_timestamp().into())
            .filter(business::Column::OwnerId.eq(user_id))
            .filter(business::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!("business update_profile_text error (user_id={user_id}): {e}");
                AppError::Internal("Failed to update profile.".into())
            })
    }

    // ── PostGIS location update — must stay as raw SQL ────────────────────────

    async fn update_location(&self, user_id: i32, point_wkt: &str) -> Result<(), AppError> {
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            UPDATE businesses
               SET location = ST_GeographyFromText($1), updated_on = NOW()
             WHERE owner_id = $2 AND deleted_at IS NULL
            "#,
            [point_wkt.into(), user_id.into()],
        );

        self.db
            .execute(stmt)
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("business update_location error (user_id={user_id}): {e}");
                AppError::Internal("Failed to update GPS location.".into())
            })
    }

    // ── Entity API helpers ────────────────────────────────────────────────────

    async fn get_business_id_by_user(&self, user_id: i32) -> Result<Option<i32>, AppError> {
        Business::find()
            .filter(business::Column::OwnerId.eq(user_id))
            .filter(business::Column::DeletedAt.is_null())
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(|b| b.id))
            .map_err(|e| {
                error!("business get_business_id_by_user error: {e}");
                AppError::Internal("Failed to resolve business.".into())
            })
    }

    async fn count_all(&self) -> Result<i64, AppError> {
        Business::find()
            .filter(business::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("business count_all error: {e}");
                AppError::Internal("Failed to load stats.".into())
            })
    }

    async fn count_verified(&self) -> Result<i64, AppError> {
        Business::find()
            .filter(business::Column::IsVerified.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("business count_verified error: {e}");
                AppError::Internal("Không thể tải thống kê.".into())
            })
    }

    async fn create_for_registration(
        &self,
        name:    &str,
        phone:   &str,
        user_id: i32,
    ) -> Result<i32, AppError> {
        let new_business = business::ActiveModel {
            name:        Set(name.to_string()),
            phone:       Set(phone.to_string()),
            owner_id:    Set(Some(user_id)),
            is_verified: Set(false),
            is_active:   Set(true),
            ..Default::default()
        };

        Business::insert(new_business)
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| {
                error!("business create_for_registration error: {e}");
                AppError::Internal("Không thể tạo hồ sơ doanh nghiệp.".into())
            })
    }
}

// ── Shared helper: load distinct service category names per business ──────────

async fn load_service_names(
    db: &DatabaseConnection,
    ids: &[i32],
) -> Result<HashMap<i32, Vec<String>>, AppError> {
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
            AppError::Internal("Failed to load services.".into())
        })?;

    let mut map: HashMap<i32, Vec<String>> = HashMap::new();
    for r in rows {
        map.entry(r.business_id).or_default().push(r.svc_name);
    }
    Ok(map)
}

