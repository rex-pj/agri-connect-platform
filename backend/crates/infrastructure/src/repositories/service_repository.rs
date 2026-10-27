use async_trait::async_trait;
use chrono::Utc;
use sea_orm::sea_query::{Alias, Expr, Func, Order, Query, SimpleExpr, WindowStatement};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, QueryTrait, RelationTrait, Select, Set, TransactionTrait,
    Value,
};
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

use domain::{
    models::{
        business::ProviderSort,
        service::{
            AdminService, NewService, ProviderService, PublicService, PublicServiceFilter,
            UpdateOwnService, UpdateService,
        },
    },
    repositories::service_repository_trait::ServiceRepositoryTrait,
};

use domain::Error;

use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        business::{self, Entity as Business},
        category::{self, Entity as Category},
        service::{self, Entity as ServiceEntity},
    },
    search::{
        search_condition, text_contains, ADMIN_SERVICE_SEARCH_COLUMNS,
        SERVICE_LISTING_SEARCH_COLUMNS,
    },
};

pub struct ServiceRepository {
    db: Arc<DatabaseConnection>,
}

impl ServiceRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[derive(FromQueryResult)]
struct AdminServiceRow {
    id: i32,
    name: String,
    description: Option<String>,
    price_note: Option<String>,
    category: String,
    category_id: i32,
    business_id: i32,
    business_name: String,
    is_active: bool,
    image_file_id: Option<Uuid>,
    created_by: Option<i32>,
    owner_id: Option<i32>,
    created_on: DateTimeWithTimeZone,
    updated_on: DateTimeWithTimeZone,
}

#[derive(FromQueryResult)]
struct ProviderServiceRow {
    id: i32,
    name: String,
    description: Option<String>,
    price_note: Option<String>,
    category: String,
    category_id: i32,
    category_icon: Option<String>,
    is_active: bool,
    image_file_id: Option<Uuid>,
    created_on: DateTimeWithTimeZone,
}

fn base_admin_service_query() -> sea_orm::Select<ServiceEntity> {
    ServiceEntity::find()
        .select_only()
        .column(service::Column::Id)
        .column(service::Column::Name)
        .column(service::Column::Description)
        .column(service::Column::PriceNote)
        .column_as(category::Column::Name, "category")
        .column(service::Column::CategoryId)
        .column(service::Column::BusinessId)
        .column_as(business::Column::Name, "business_name")
        .column(service::Column::IsActive)
        .column(service::Column::ImageFileId)
        .column(service::Column::CreatedBy)
        .column(business::Column::OwnerId)
        .column(service::Column::CreatedOn)
        .column(service::Column::UpdatedOn)
        .join(JoinType::InnerJoin, service::Relation::Business.def())
        .join(JoinType::InnerJoin, service::Relation::Category.def())
        .filter(service::Column::DeletedAt.is_null())
        .filter(business::Column::DeletedAt.is_null())
}

fn base_provider_service_query() -> sea_orm::Select<ServiceEntity> {
    ServiceEntity::find()
        .select_only()
        .column(service::Column::Id)
        .column(service::Column::Name)
        .column(service::Column::Description)
        .column(service::Column::PriceNote)
        .column_as(category::Column::Name, "category")
        .column(service::Column::CategoryId)
        .column_as(category::Column::Icon, "category_icon")
        .column(service::Column::IsActive)
        .column(service::Column::ImageFileId)
        .column(service::Column::CreatedOn)
        .join(JoinType::InnerJoin, service::Relation::Category.def())
        .filter(service::Column::DeletedAt.is_null())
}

/// Services of a live business `owner_user` owns: a subquery, so the
/// ownership check and the read or write are one statement.
fn owned_by(owner_user: i32) -> SimpleExpr {
    Expr::col((service::Entity, service::Column::BusinessId)).in_subquery(
        Query::select()
            .column((business::Entity, business::Column::Id))
            .from(business::Entity)
            .and_where(Expr::col((business::Entity, business::Column::OwnerId)).eq(owner_user))
            .and_where(Expr::col((business::Entity, business::Column::DeletedAt)).is_null())
            .to_owned(),
    )
}

fn map_admin_row(r: AdminServiceRow) -> AdminService {
    AdminService {
        id: r.id,
        name: r.name,
        description: r.description,
        price_note: r.price_note,
        category: r.category,
        category_id: r.category_id,
        business_id: r.business_id,
        business_name: r.business_name,
        is_active: r.is_active,
        image_file_id: r.image_file_id,
        created_by: r.created_by,
        owner_id: r.owner_id,
        created_on: r.created_on.with_timezone(&Utc),
        updated_on: r.updated_on.with_timezone(&Utc),
    }
}

fn map_provider_row(r: ProviderServiceRow) -> ProviderService {
    ProviderService {
        id: r.id,
        name: r.name,
        description: r.description,
        price_note: r.price_note,
        category: r.category,
        category_id: r.category_id,
        category_icon: r.category_icon,
        is_active: r.is_active,
        image_file_id: r.image_file_id,
        created_on: r.created_on.with_timezone(&Utc),
    }
}

#[derive(FromQueryResult)]
struct PublicServiceRow {
    id: i32,
    name: String,
    description: Option<String>,
    price_note: Option<String>,
    category_id: i32,
    category: String,
    category_slug: String,
    category_icon: Option<String>,
    image_file_id: Option<Uuid>,
    business_id: i32,
    business_name: String,
    business_phone: String,
    business_address: Option<String>,
    business_verified: bool,
    created_on: DateTimeWithTimeZone,
}

#[derive(FromQueryResult)]
struct GeoServiceRow {
    #[sea_orm(nested)]
    service: PublicServiceRow,
    distance_m: f64,
}

/// The joins and conditions that make a service public: shown and not
/// deleted, in a shown category, offered by a live business.
fn live_public(query: Select<ServiceEntity>) -> Select<ServiceEntity> {
    query
        .join(JoinType::InnerJoin, service::Relation::Business.def())
        .join(JoinType::InnerJoin, service::Relation::Category.def())
        .filter(service::Column::IsActive.eq(true))
        .filter(service::Column::DeletedAt.is_null())
        .filter(category::Column::IsActive.eq(true))
        .filter(business::Column::IsActive.eq(true))
        .filter(business::Column::DeletedAt.is_null())
}

fn base_public_query() -> Select<ServiceEntity> {
    live_public(
        ServiceEntity::find()
            .select_only()
            .column(service::Column::Id)
            .column(service::Column::Name)
            .column(service::Column::Description)
            .column(service::Column::PriceNote)
            .column(service::Column::CategoryId)
            .column_as(category::Column::Name, "category")
            .column_as(category::Column::Slug, "category_slug")
            .column_as(category::Column::Icon, "category_icon")
            .column(service::Column::ImageFileId)
            .column(service::Column::BusinessId)
            .column_as(business::Column::Name, "business_name")
            .column_as(business::Column::Phone, "business_phone")
            .column_as(business::Column::Address, "business_address")
            .column_as(business::Column::IsVerified, "business_verified")
            .column(service::Column::CreatedOn),
    )
}

/// The output column holding a service's turn within its provider.
const PROVIDER_TURN: &str = "provider_turn";

/// Adds each service's turn within its provider (1 for its newest), so a list
/// ordered by it first shows every provider once before anyone's second.
fn with_provider_turn(mut query: Select<ServiceEntity>) -> Select<ServiceEntity> {
    QueryTrait::query(&mut query).expr_window_as(
        Func::cust(Alias::new("ROW_NUMBER")),
        WindowStatement::partition_by((service::Entity, service::Column::BusinessId))
            .order_by((service::Entity, service::Column::CreatedOn), Order::Desc)
            .order_by((service::Entity, service::Column::Id), Order::Desc)
            .to_owned(),
        Alias::new(PROVIDER_TURN),
    );
    query
}
/// Narrows a public query to a category and a typed text.
fn public_filtered(
    mut query: Select<ServiceEntity>,
    filter: &PublicServiceFilter,
) -> Select<ServiceEntity> {
    if let Some(id) = filter.category_id {
        query = query.filter(service::Column::CategoryId.eq(id));
    }
    if let Some(text) = filter.text.as_deref().filter(|t| !t.trim().is_empty()) {
        query = query.filter(text_contains(SERVICE_LISTING_SEARCH_COLUMNS, text));
    }
    query
}

fn map_public_row(r: PublicServiceRow, distance_m: Option<f64>) -> PublicService {
    PublicService {
        id: r.id,
        name: r.name,
        description: r.description,
        price_note: r.price_note,
        category_id: r.category_id,
        category: r.category,
        category_slug: r.category_slug,
        category_icon: r.category_icon,
        image_file_id: r.image_file_id,
        business_id: r.business_id,
        business_name: r.business_name,
        business_phone: r.business_phone,
        business_address: r.business_address,
        business_verified: r.business_verified,
        created_on: r.created_on.with_timezone(&Utc),
        distance_m,
    }
}
fn new_service_model(service: NewService) -> service::ActiveModel {
    service::ActiveModel {
        business_id: Set(service.business_id),
        category_id: Set(service.category_id),
        name: Set(service.name),
        description: Set(service.description),
        price_note: Set(service.price_note),
        image_file_id: Set(service.image_file_id),
        created_by: Set(Some(service.created_by)),
        ..Default::default()
    }
}

#[async_trait]
impl ServiceRepositoryTrait for ServiceRepository {
    async fn list_admin(
        &self,
        provider_id: Option<i32>,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AdminService>, Error> {
        let mut query = base_admin_service_query().filter(search_condition(ADMIN_SERVICE_SEARCH_COLUMNS, search));
        if let Some(bid) = provider_id {
            query = query.filter(service::Column::BusinessId.eq(bid));
        }
        query
            .order_by_desc(service::Column::CreatedOn)
            .order_by_desc(service::Column::Id)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<AdminServiceRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(map_admin_row).collect())
            .map_err(|e| {
                error!("service list_admin error: {e}");
                Error::internal("Failed to load services.")
            })
    }

    async fn count_admin(&self, provider_id: Option<i32>, search: Option<&str>) -> Result<i64, Error> {
        let mut query = ServiceEntity::find()
            .join(JoinType::InnerJoin, service::Relation::Business.def())
            .filter(service::Column::DeletedAt.is_null())
            .filter(business::Column::DeletedAt.is_null())
            .filter(search_condition(ADMIN_SERVICE_SEARCH_COLUMNS, search));
        if let Some(bid) = provider_id {
            query = query.filter(service::Column::BusinessId.eq(bid));
        }
        query
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("service count_admin error: {e}");
                Error::internal("Failed to count services.")
            })
    }

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<AdminService>, Error> {
        base_admin_service_query()
            .filter(service::Column::Id.eq(id))
            .into_model::<AdminServiceRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(map_admin_row))
            .map_err(|e| {
                error!("service find_by_id_admin error (id={id}): {e}");
                Error::internal("Failed to load service.")
            })
    }

    async fn create(&self, service: NewService) -> Result<i32, Error> {
        ServiceEntity::insert(new_service_model(service))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| {
                error!("service create error: {e}");
                Error::internal("Failed to create service.")
            })
    }

    async fn create_capped(&self, service: NewService, cap: u64) -> Result<Option<i32>, Error> {
        let fail = |e: sea_orm::DbErr| Error::internal(format!("service create_capped: {e}"));
        let business_id = service.business_id;
        let txn = self.db.begin().await.map_err(fail)?;
        // The lock makes concurrent creates for one business count in turn.
        Business::find_by_id(business_id)
            .select_only()
            .column(business::Column::Id)
            .lock_exclusive()
            .into_tuple::<i32>()
            .one(&txn)
            .await
            .map_err(fail)?;
        let existing = ServiceEntity::find()
            .filter(service::Column::BusinessId.eq(business_id))
            .filter(service::Column::DeletedAt.is_null())
            .count(&txn)
            .await
            .map_err(fail)?;
        if existing >= cap {
            txn.rollback().await.map_err(fail)?;
            return Ok(None);
        }
        let id = ServiceEntity::insert(new_service_model(service))
            .exec(&txn)
            .await
            .map_err(fail)?
            .last_insert_id;
        txn.commit().await.map_err(fail)?;
        Ok(Some(id))
    }

    async fn update(&self, service: UpdateService) -> Result<bool, Error> {
        ServiceEntity::update_many()
            .col_expr(
                service::Column::CategoryId,
                Expr::value(service.category_id),
            )
            .col_expr(service::Column::Name, Expr::value(service.name))
            .col_expr(
                service::Column::Description,
                Expr::value(service.description),
            )
            .col_expr(service::Column::PriceNote, Expr::value(service.price_note))
            .col_expr(
                service::Column::UpdatedBy,
                Expr::value(Some(service.updated_by)),
            )
            .col_expr(service::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(service::Column::Id.eq(service.id))
            .filter(service::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!("service update error (id={}): {e}", service.id);
                Error::internal("Failed to update service.")
            })
    }

    async fn toggle_active(&self, id: i32, is_active: bool) -> Result<bool, Error> {
        ServiceEntity::update_many()
            .col_expr(service::Column::IsActive, Expr::value(is_active))
            .col_expr(service::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(service::Column::Id.eq(id))
            .filter(service::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!("service toggle_active error (id={id}): {e}");
                Error::internal("Failed to update service.")
            })
    }

    async fn delete(&self, id: i32) -> Result<bool, Error> {
        ServiceEntity::update_many()
            .col_expr(service::Column::DeletedAt, Expr::current_timestamp().into())
            .filter(service::Column::Id.eq(id))
            .filter(service::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!("service delete error (id={id}): {e}");
                Error::internal("Failed to delete service.")
            })
    }

    async fn active_service_category_name(
        &self,
        category_id: i32,
    ) -> Result<Option<String>, Error> {
        Category::find_by_id(category_id)
            .select_only()
            .column(category::Column::Name)
            .filter(category::Column::Type.eq(domain::constants::CATEGORY_TYPE_SERVICE))
            .filter(category::Column::IsActive.eq(true))
            .into_tuple::<String>()
            .one(self.db.as_ref())
            .await
            .map_err(|e| Error::internal(format!("service category check: {e}")))
    }

    async fn list_by_owner(&self, owner_user: i32) -> Result<Vec<ProviderService>, Error> {
        base_provider_service_query()
            .filter(owned_by(owner_user))
            .order_by_desc(service::Column::CreatedOn)
            .order_by_desc(service::Column::Id)
            .into_model::<ProviderServiceRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(map_provider_row).collect())
            .map_err(|e| Error::internal(format!("service list_by_owner (user={owner_user}): {e}")))
    }

    async fn find_owned(&self, id: i32, owner_user: i32) -> Result<Option<ProviderService>, Error> {
        base_provider_service_query()
            .filter(service::Column::Id.eq(id))
            .filter(owned_by(owner_user))
            .into_model::<ProviderServiceRow>()
            .one(self.db.as_ref())
            .await
            .map(|row| row.map(map_provider_row))
            .map_err(|e| Error::internal(format!("service find_owned (id={id}): {e}")))
    }

    async fn update_owned(&self, service: UpdateOwnService) -> Result<bool, Error> {
        let id = service.id;
        ServiceEntity::update_many()
            .col_expr(
                service::Column::CategoryId,
                Expr::value(service.category_id),
            )
            .col_expr(service::Column::Name, Expr::value(service.name))
            .col_expr(
                service::Column::Description,
                Expr::value(service.description),
            )
            .col_expr(service::Column::PriceNote, Expr::value(service.price_note))
            .col_expr(
                service::Column::UpdatedBy,
                Expr::value(Some(service.owner_user)),
            )
            .col_expr(service::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(service::Column::Id.eq(id))
            .filter(service::Column::DeletedAt.is_null())
            .filter(owned_by(service.owner_user))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| Error::internal(format!("service update_owned (id={id}): {e}")))
    }

    async fn soft_delete_owned(&self, id: i32, owner_user: i32) -> Result<bool, Error> {
        ServiceEntity::update_many()
            .col_expr(service::Column::DeletedAt, Expr::current_timestamp().into())
            .col_expr(service::Column::UpdatedBy, Expr::value(Some(owner_user)))
            .filter(service::Column::Id.eq(id))
            .filter(service::Column::DeletedAt.is_null())
            .filter(owned_by(owner_user))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| Error::internal(format!("service soft_delete_owned (id={id}): {e}")))
    }

    async fn set_image_owned(
        &self,
        id: i32,
        owner_user: i32,
        file_id: Uuid,
    ) -> Result<Option<Option<Uuid>>, Error> {
        let fail = |e: sea_orm::DbErr| Error::internal(format!("service set_image (id={id}): {e}"));
        let txn = self.db.begin().await.map_err(fail)?;
        // FOR UPDATE: a concurrent upload waits here, then reads this
        // upload's image as its previous one, so neither is orphaned.
        let previous = ServiceEntity::find_by_id(id)
            .select_only()
            .column(service::Column::ImageFileId)
            .filter(service::Column::DeletedAt.is_null())
            .filter(owned_by(owner_user))
            .lock_exclusive()
            .into_tuple::<Option<Uuid>>()
            .one(&txn)
            .await
            .map_err(fail)?;
        let Some(previous) = previous else {
            txn.rollback().await.map_err(fail)?;
            return Ok(None);
        };
        ServiceEntity::update_many()
            .col_expr(service::Column::ImageFileId, Expr::value(Some(file_id)))
            .col_expr(service::Column::UpdatedBy, Expr::value(Some(owner_user)))
            .col_expr(service::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(service::Column::Id.eq(id))
            .exec(&txn)
            .await
            .map_err(fail)?;
        txn.commit().await.map_err(fail)?;
        Ok(Some(previous))
    }

    async fn image_of(&self, id: i32) -> Result<Option<Uuid>, Error> {
        ServiceEntity::find_by_id(id)
            .select_only()
            .column(service::Column::ImageFileId)
            .into_tuple::<Option<Uuid>>()
            .one(self.db.as_ref())
            .await
            .map(Option::flatten)
            .map_err(|e| Error::internal(format!("service image_of (id={id}): {e}")))
    }

    async fn browse_public(
        &self,
        filter: &PublicServiceFilter,
        sort: ProviderSort,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PublicService>, Error> {
        let query = public_filtered(base_public_query(), filter);
        let query = match sort {
            ProviderSort::Newest => query.order_by_desc(service::Column::CreatedOn),
            ProviderSort::NameAsc => query.order_by_asc(service::Column::Name),
            // The default order takes turns: every provider's first service,
            // then every provider's second, so one shop never fills a page.
            ProviderSort::VerifiedFirst => with_provider_turn(query)
                .order_by(Expr::col(Alias::new(PROVIDER_TURN)), Order::Asc)
                .order_by_desc(business::Column::IsVerified)
                .order_by_desc(service::Column::CreatedOn),
        };
        query
            // A fixed order for ties, so no row repeats or goes missing across pages.
            .order_by_desc(service::Column::Id)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<PublicServiceRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(|r| map_public_row(r, None)).collect())
            .map_err(|e| Error::internal(format!("service browse_public: {e}")))
    }

    async fn count_public(&self, filter: &PublicServiceFilter) -> Result<i64, Error> {
        public_filtered(live_public(ServiceEntity::find()), filter)
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| Error::internal(format!("service count_public: {e}")))
    }

    async fn find_public(&self, id: i32) -> Result<Option<PublicService>, Error> {
        base_public_query()
            .filter(service::Column::Id.eq(id))
            .into_model::<PublicServiceRow>()
            .one(self.db.as_ref())
            .await
            .map(|row| row.map(|r| map_public_row(r, None)))
            .map_err(|e| Error::internal(format!("service find_public (id={id}): {e}")))
    }

    async fn featured_candidates(&self, limit: i64) -> Result<Vec<PublicService>, Error> {
        base_public_query()
            .filter(business::Column::IsVerified.eq(true))
            .order_by(
                Expr::col((service::Entity, service::Column::ImageFileId)).is_null(),
                Order::Asc,
            )
            .order_by_desc(service::Column::CreatedOn)
            .order_by_desc(service::Column::Id)
            .limit(limit as u64)
            .into_model::<PublicServiceRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(|r| map_public_row(r, None)).collect())
            .map_err(|e| Error::internal(format!("service featured_candidates: {e}")))
    }

    // PostGIS expressions via Expr::cust_with_values, the one thing the
    // builder cannot express; placeholders are `$1`, `$2` (see the business
    // repository's geo search).
    async fn search_geo_public(
        &self,
        point_wkt: &str,
        radius: f64,
        category_id: Option<i32>,
        limit: i64,
    ) -> Result<Vec<PublicService>, Error> {
        let distance = Expr::cust_with_values(
            r#"ST_Distance("businesses".location, ST_GeographyFromText($1))"#,
            [Value::from(point_wkt.to_owned())],
        );
        let within = Expr::cust_with_values(
            r#"ST_DWithin("businesses".location, ST_GeographyFromText($1), $2)"#,
            [Value::from(point_wkt.to_owned()), Value::from(radius)],
        );
        let filter = PublicServiceFilter {
            category_id,
            text: None,
        };
        // Nearest first, taking turns: each provider's first service before
        // anyone's second (a provider's services all share its distance).
        let mut query = with_provider_turn(public_filtered(base_public_query(), &filter))
            .expr_as(distance, "distance_m")
            .filter(within)
            .order_by(Expr::col(Alias::new(PROVIDER_TURN)), Order::Asc)
            .limit(limit as u64);
        QueryTrait::query(&mut query).order_by_expr(Expr::cust("distance_m"), Order::Asc);
        query
            .order_by_asc(service::Column::Id)
            .into_model::<GeoServiceRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| map_public_row(r.service, Some(r.distance_m)))
                    .collect()
            })
            .map_err(|e| Error::internal(format!("service search_geo_public: {e}")))
    }
}
