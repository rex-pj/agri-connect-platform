use async_trait::async_trait;
use chrono::Utc;
use sea_orm::sea_query::{Expr, Func, Order, SimpleExpr, WindowStatement};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, QueryTrait, RelationTrait, Select, Set, TransactionTrait,
};
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

use domain::{
    models::{
        admin_list::AdminFilter,
        business::{ListingEditor, ProviderSort},
        service::{
            AdminService, NewService, ProviderService, PublicService, PublicServiceFilter,
            UpdateOwnService, UpdateService,
        },
    },
    repositories::service_repository_trait::ServiceRepositoryTrait,
};

use domain::Error;

use crate::{
    admin_list::{admin_order, shown_state, OrderColumns},
    db_types::DateTimeWithTimeZone,
    entities::{
        business,
        service::{self, Entity as ServiceEntity},
        service_type::{self as category, Entity as Category},
    },
    repositories::lock_business,
    search::{admin_service_search, service_listing_matches},
    sql::{
        business_location, businesses_of, category_in_sector, distance, geography_from_text,
        output, within, DbFn, QueryAs,
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
    sector: Option<String>,
    business_id: i32,
    business_name: String,
    is_active: bool,
    business_active: bool,
    category_active: bool,
    business_phone: String,
    business_verified: bool,
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
        .column_as(category::Column::Name, QueryAs::Category)
        .column_as(service::Column::ServiceTypeId, QueryAs::CategoryId)
        .column(service::Column::BusinessId)
        .column_as(category::Column::Sector, QueryAs::Sector)
        .column_as(business::Column::Name, QueryAs::BusinessName)
        .column(service::Column::IsActive)
        .column_as(business::Column::IsActive, QueryAs::BusinessActive)
        .column_as(category::Column::IsActive, QueryAs::CategoryActive)
        .column_as(business::Column::Phone, QueryAs::BusinessPhone)
        .column_as(business::Column::IsVerified, QueryAs::BusinessVerified)
        .column(service::Column::ImageFileId)
        .column(service::Column::CreatedBy)
        .column(business::Column::OwnerId)
        .column(service::Column::CreatedOn)
        .column(service::Column::UpdatedOn)
        .join(JoinType::InnerJoin, service::Relation::Business.def())
        .join(JoinType::InnerJoin, service::Relation::ServiceType.def())
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
        .column_as(category::Column::Name, QueryAs::Category)
        .column_as(service::Column::ServiceTypeId, QueryAs::CategoryId)
        .column_as(category::Column::Icon, QueryAs::CategoryIcon)
        .column(service::Column::IsActive)
        .column(service::Column::ImageFileId)
        .column(service::Column::CreatedOn)
        .join(JoinType::InnerJoin, service::Relation::ServiceType.def())
        .filter(service::Column::DeletedAt.is_null())
}

/// Services of a live business `owner_user` owns: a subquery, so the
/// ownership check and the read or write are one statement.
fn owned_by(owner_user: i32) -> SimpleExpr {
    Expr::col((service::Entity, service::Column::BusinessId)).in_subquery(businesses_of(owner_user))
}

fn map_admin_row(r: AdminServiceRow) -> AdminService {
    AdminService {
        id: r.id,
        name: r.name,
        description: r.description,
        price_note: r.price_note,
        category: r.category,
        category_id: r.category_id,
        sector: r.sector,
        business_id: r.business_id,
        business_name: r.business_name,
        is_active: r.is_active,
        business_active: r.business_active,
        category_active: r.category_active,
        business_phone: r.business_phone,
        business_verified: r.business_verified,
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
pub(crate) fn live_public(query: Select<ServiceEntity>) -> Select<ServiceEntity> {
    query
        .join(JoinType::InnerJoin, service::Relation::Business.def())
        .join(JoinType::InnerJoin, service::Relation::ServiceType.def())
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
            .column_as(service::Column::ServiceTypeId, QueryAs::CategoryId)
            .column_as(category::Column::Name, QueryAs::Category)
            .column_as(category::Column::Slug, QueryAs::CategorySlug)
            .column_as(category::Column::Icon, QueryAs::CategoryIcon)
            .column(service::Column::ImageFileId)
            .column(service::Column::BusinessId)
            .column_as(business::Column::Name, QueryAs::BusinessName)
            .column_as(business::Column::Phone, QueryAs::BusinessPhone)
            .column_as(business::Column::Address, QueryAs::BusinessAddress)
            .column_as(business::Column::IsVerified, QueryAs::BusinessVerified)
            .column(service::Column::CreatedOn),
    )
}

/// Adds each service's turn within its provider (1 for its newest), so a list
/// ordered by it first shows every provider once before anyone's second.
fn with_provider_turn(mut query: Select<ServiceEntity>) -> Select<ServiceEntity> {
    QueryTrait::query(&mut query).expr_window_as(
        Func::cust(DbFn::RowNumber),
        WindowStatement::partition_by((service::Entity, service::Column::BusinessId))
            .order_by((service::Entity, service::Column::CreatedOn), Order::Desc)
            .order_by((service::Entity, service::Column::Id), Order::Desc)
            .to_owned(),
        QueryAs::ProviderTurn,
    );
    query
}
/// Narrows a public query to a category and a typed text.
fn public_filtered(
    mut query: Select<ServiceEntity>,
    filter: &PublicServiceFilter,
) -> Select<ServiceEntity> {
    if let Some(id) = filter.category_id {
        query = query.filter(service::Column::ServiceTypeId.eq(id));
    }
    if let Some(sector) = filter.sector.as_deref() {
        query = query.filter(category_in_sector(sector));
    }
    if let Some(text) = filter.text.as_deref().filter(|t| !t.trim().is_empty()) {
        query = query.filter(service_listing_matches(text));
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
        service_type_id: Set(service.category_id),
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
        filter: &AdminFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AdminService>, Error> {
        let query = base_admin_service_query().filter(admin_condition(filter));
        admin_order(
            query,
            filter.sort,
            OrderColumns {
                created: service::Column::CreatedOn,
                name: service::Column::Name,
                id: service::Column::Id,
            },
            |q| {
                q.order_by_desc(service::Column::CreatedOn)
                    .order_by_desc(service::Column::Id)
            },
        )
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

    async fn count_admin(&self, filter: &AdminFilter) -> Result<i64, Error> {
        ServiceEntity::find()
            .join(JoinType::InnerJoin, service::Relation::Business.def())
            .filter(service::Column::DeletedAt.is_null())
            .filter(business::Column::DeletedAt.is_null())
            .filter(admin_condition(filter))
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
        lock_business(&txn, business_id).await.map_err(fail)?;
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
                service::Column::ServiceTypeId,
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
                service::Column::ServiceTypeId,
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

    async fn set_image(
        &self,
        id: i32,
        editor: ListingEditor,
        file_id: Option<Uuid>,
    ) -> Result<Option<Option<Uuid>>, Error> {
        let fail = |e: sea_orm::DbErr| Error::internal(format!("service set_image (id={id}): {e}"));
        let txn = self.db.begin().await.map_err(fail)?;
        // FOR UPDATE: a concurrent upload waits here, then reads this
        // upload's image as its previous one, so neither is orphaned.
        let mut query = ServiceEntity::find_by_id(id)
            .select_only()
            .column(service::Column::ImageFileId)
            .filter(service::Column::DeletedAt.is_null());
        if let ListingEditor::Owner(owner_user) = editor {
            query = query.filter(owned_by(owner_user));
        }
        let previous = query
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
            .col_expr(service::Column::ImageFileId, Expr::value(file_id))
            .col_expr(
                service::Column::UpdatedBy,
                Expr::value(Some(editor.user_id())),
            )
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
                .order_by(output(QueryAs::ProviderTurn), Order::Asc)
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

    async fn featured_candidates(
        &self,
        limit: i64,
        sector: Option<&str>,
    ) -> Result<Vec<PublicService>, Error> {
        let filter = PublicServiceFilter {
            sector: sector.map(str::to_owned),
            ..PublicServiceFilter::default()
        };
        public_filtered(base_public_query(), &filter)
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

    // PostGIS calls built from `sql`, as in the business repository's geo search.
    async fn search_geo_public(
        &self,
        point_wkt: &str,
        radius: f64,
        category_id: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PublicService>, Error> {
        let distance = distance(business_location(), geography_from_text(point_wkt));
        // Nearest first, taking turns: each provider's first service before
        // anyone's second (a provider's services all share its distance).
        let mut query = with_provider_turn(near_public(
            base_public_query(),
            point_wkt,
            radius,
            category_id,
        ))
        .expr_as(distance, QueryAs::DistanceM)
        .order_by(output(QueryAs::ProviderTurn), Order::Asc)
        .limit(limit as u64)
        .offset(offset as u64);
        QueryTrait::query(&mut query).order_by_expr(output(QueryAs::DistanceM), Order::Asc);
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

    async fn count_geo_public(
        &self,
        point_wkt: &str,
        radius: f64,
        category_id: Option<i32>,
    ) -> Result<i64, Error> {
        near_public(
            live_public(ServiceEntity::find()),
            point_wkt,
            radius,
            category_id,
        )
        .count(self.db.as_ref())
        .await
        .map(|c| c as i64)
        .map_err(|e| Error::internal(format!("service count_geo_public: {e}")))
    }
}

/// A public query narrowed to the services within `radius` metres of
/// `point_wkt` (`category_id`: of that category): what the geo search pages
/// through and what its count counts, the one definition.
fn near_public(
    query: Select<ServiceEntity>,
    point_wkt: &str,
    radius: f64,
    category_id: Option<i32>,
) -> Select<ServiceEntity> {
    let filter = PublicServiceFilter {
        category_id,
        sector: None,
        text: None,
    };
    public_filtered(query, &filter).filter(within(
        business_location(),
        geography_from_text(point_wkt),
        radius,
    ))
}

/// The admin list's criteria, shared by the list and its count.
fn admin_condition(filter: &AdminFilter) -> sea_orm::Condition {
    admin_condition_base(filter).add(crate::admin_list::created_between(
        (
            crate::entities::service::Entity,
            crate::entities::service::Column::CreatedOn,
        ),
        filter,
    ))
}

fn admin_condition_base(filter: &AdminFilter) -> sea_orm::Condition {
    let mut cond = sea_orm::Condition::all()
        .add(admin_service_search(filter.search()))
        .add(shown_state(service::Column::IsActive, filter.state));
    if let Some(bid) = filter.provider_id {
        cond = cond.add(service::Column::BusinessId.eq(bid));
    }
    if let Some(sector) = filter.sector.as_deref() {
        cond = cond.add(crate::sql::category_in_sector(sector));
    }
    if filter.gap.as_deref() == Some("no_image") {
        cond = cond.add(service::Column::ImageFileId.is_null());
    }
    if let Some(id) = filter.category_id {
        cond = cond.add(service::Column::ServiceTypeId.eq(id));
    }
    cond
}
