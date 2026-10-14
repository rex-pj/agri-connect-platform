use async_trait::async_trait;
use chrono::Utc;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set,
};
use std::sync::Arc;
use tracing::error;

use domain::{
    models::service::{AdminService, NewService, UpdateService},
    repositories::service_repository_trait::ServiceRepositoryTrait,
};

use domain::Error;

use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        business,
        category::{self, Entity as Category},
        service::{self, Entity as ServiceEntity},
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
    created_on: DateTimeWithTimeZone,
    updated_on: DateTimeWithTimeZone,
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
        .column(service::Column::CreatedOn)
        .column(service::Column::UpdatedOn)
        .join(JoinType::InnerJoin, service::Relation::Business.def())
        .join(JoinType::InnerJoin, service::Relation::Category.def())
        .filter(service::Column::DeletedAt.is_null())
        .filter(business::Column::DeletedAt.is_null())
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
        created_on: r.created_on.with_timezone(&Utc),
        updated_on: r.updated_on.with_timezone(&Utc),
    }
}

#[async_trait]
impl ServiceRepositoryTrait for ServiceRepository {
    async fn list_admin(
        &self,
        provider_id: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AdminService>, Error> {
        let mut query = base_admin_service_query();
        if let Some(bid) = provider_id {
            query = query.filter(service::Column::BusinessId.eq(bid));
        }
        query
            .order_by_desc(service::Column::CreatedOn)
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

    async fn count_admin(&self, provider_id: Option<i32>) -> Result<i64, Error> {
        let mut query = ServiceEntity::find()
            .join(JoinType::InnerJoin, service::Relation::Business.def())
            .filter(service::Column::DeletedAt.is_null())
            .filter(business::Column::DeletedAt.is_null());
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
        let model = service::ActiveModel {
            business_id: Set(service.business_id),
            category_id: Set(service.category_id),
            name: Set(service.name),
            description: Set(service.description),
            price_note: Set(service.price_note),
            created_by: Set(Some(service.created_by)),
            ..Default::default()
        };
        ServiceEntity::insert(model)
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| {
                error!("service create error: {e}");
                Error::internal("Failed to create service.")
            })
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

    async fn check_category_is_service(&self, category_id: i32) -> Result<bool, Error> {
        Category::find_by_id(category_id)
            .filter(category::Column::Type.eq("service"))
            .filter(category::Column::IsActive.eq(true))
            .count(self.db.as_ref())
            .await
            .map(|c| c > 0)
            .map_err(|e| {
                error!("service check_category_is_service error: {e}");
                Error::internal("Failed to check category.")
            })
    }
}
