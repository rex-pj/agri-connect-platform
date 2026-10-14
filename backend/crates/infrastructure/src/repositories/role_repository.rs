use async_trait::async_trait;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use std::sync::Arc;
use tracing::error;

use domain::{models::role::RoleModel, repositories::role_repository_trait::RoleRepositoryTrait};

use domain::Error;

use crate::{
    entities::role::{self, Entity as Role},
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
    async fn list(&self) -> Result<Vec<RoleModel>, Error> {
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
                Error::internal("Failed to load roles.")
            })
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<RoleModel>, Error> {
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
                Error::internal("Failed to load role.")
            })
    }

    async fn find_active_by_code(&self, code: &str) -> Result<Option<RoleModel>, Error> {
        Role::find()
            .filter(role::Column::Code.eq(code))
            .filter(role::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(to_model))
            .map_err(|e| Error::internal(format!("role find_active_by_code: {e}")))
    }
}

fn to_model(r: role::Model) -> RoleModel {
    RoleModel { id: r.id, code: r.code, name: r.name, description: r.description, is_active: r.is_active }
}
