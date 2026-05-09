use async_trait::async_trait;
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder};
use std::sync::Arc;
use tracing::error;

use crate::{
    domain::{models::role::RoleModel, repositories::role_repository_trait::RoleRepositoryTrait},
    entities::role::{self, Entity as Role},
    view_models::AppError,
};

pub struct RoleRepository {
    db: Arc<DatabaseConnection>,
}

impl RoleRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RoleRepositoryTrait for RoleRepository {
    async fn list(&self) -> Result<Vec<RoleModel>, AppError> {
        Role::find()
            .order_by_asc(role::Column::Id)
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| RoleModel {
                        id: r.id,
                        code: r.code,
                        name: r.name,
                        description: r.description,
                        is_active: r.is_active,
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("role list error: {e}");
                AppError::Internal("Failed to load roles.".into())
            })
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<RoleModel>, AppError> {
        Role::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| RoleModel {
                    id: r.id,
                    code: r.code,
                    name: r.name,
                    description: r.description,
                    is_active: r.is_active,
                })
            })
            .map_err(|e| {
                error!("role find_by_id error: {e}");
                AppError::Internal("Failed to load role.".into())
            })
    }
}
