use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    Statement, TransactionTrait,
};
use std::sync::Arc;
use tracing::error;

use domain::{
    models::role::RoleModel,
    repositories::role_repository_trait::{
        GrantChanges, RoleGrantsRepositoryTrait, RoleRepositoryTrait,
    },
};

use domain::Error;

use crate::entities::role::{self, Entity as Role};

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
    RoleModel {
        id: r.id,
        code: r.code,
        name: r.name,
        description: r.description,
        is_active: r.is_active,
    }
}

#[async_trait]
impl RoleGrantsRepositoryTrait for RoleRepository {
    async fn replace(
        &self,
        role_id: i32,
        keys: &[String],
        granted_by: i32,
    ) -> Result<GrantChanges, Error> {
        let fail =
            |e: sea_orm::DbErr| Error::internal(format!("role grants (role {role_id}): {e}"));
        let backend = self.db.get_database_backend();
        let keys = keys.to_vec();
        // One transaction: a reader sees the old grants or the new ones,
        // never a role with half its permissions removed.
        let txn = self.db.begin().await.map_err(fail)?;
        let removed: Vec<String> = txn
            .query_all(Statement::from_sql_and_values(
                backend,
                "WITH gone AS (
                     DELETE FROM role_permissions rp USING permissions p
                     WHERE rp.permission_id = p.id AND rp.role_id = $1
                       AND NOT (p.code = ANY($2))
                     RETURNING p.code)
                 SELECT code FROM gone ORDER BY code",
                [role_id.into(), keys.clone().into()],
            ))
            .await
            .map_err(fail)?
            .iter()
            .filter_map(|r| r.try_get::<String>("", "code").ok())
            .collect();
        let added: Vec<String> = txn
            .query_all(Statement::from_sql_and_values(
                backend,
                "WITH new AS (
                     INSERT INTO role_permissions (role_id, permission_id, granted_by)
                     SELECT $1, p.id, $3 FROM permissions p WHERE p.code = ANY($2)
                     ON CONFLICT (role_id, permission_id) DO NOTHING
                     RETURNING permission_id)
                 SELECT p.code FROM new JOIN permissions p ON p.id = new.permission_id
                 ORDER BY p.code",
                [role_id.into(), keys.into(), granted_by.into()],
            ))
            .await
            .map_err(fail)?
            .iter()
            .filter_map(|r| r.try_get::<String>("", "code").ok())
            .collect();
        txn.commit().await.map_err(fail)?;
        Ok(GrantChanges { added, removed })
    }
}
