use std::sync::Arc;

use async_trait::async_trait;
use domain::{
    models::{permission::PermissionModel, user::ActorRecord},
    repositories::permission_repository_trait::PermissionRepositoryTrait,
    Error,
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QueryOrder, Statement,
};

use crate::entities::permission::{self, Entity as Permission};

pub struct PermissionRepository {
    db: Arc<DatabaseConnection>,
}

impl PermissionRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    async fn query_models(
        &self,
        sql: &str,
        value: i32,
        what: &str,
    ) -> Result<Vec<PermissionModel>, Error> {
        let db = self.db.as_ref();
        PermissionRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            sql,
            [value.into()],
        ))
        .all(db)
        .await
        .map(|rows| rows.into_iter().map(PermissionModel::from).collect())
        .map_err(|e| db_error(what, e))
    }
}

#[derive(FromQueryResult)]
struct PermissionRow {
    id: i32,
    code: String,
    name: String,
}

impl From<PermissionRow> for PermissionModel {
    fn from(r: PermissionRow) -> Self {
        Self {
            id: r.id,
            code: r.code,
            name: r.name,
        }
    }
}

#[derive(FromQueryResult)]
struct ActorRow {
    id: i32,
    username: String,
    role_code: String,
    is_active: bool,
}

#[derive(FromQueryResult)]
struct ExistsRow {
    granted: bool,
}

fn db_error(what: &str, e: DbErr) -> Error {
    Error::internal(format!("permission {what}: {e}"))
}

#[async_trait]
impl PermissionRepositoryTrait for PermissionRepository {
    async fn list_all(&self) -> Result<Vec<PermissionModel>, Error> {
        Permission::find()
            .order_by_asc(permission::Column::Code)
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| PermissionModel {
                        id: r.id,
                        code: r.code,
                        name: r.name,
                    })
                    .collect()
            })
            .map_err(|e| db_error("list_all", e))
    }

    async fn list_for_role(&self, role_id: i32) -> Result<Vec<PermissionModel>, Error> {
        self.query_models(
            "SELECT p.id, p.code, p.name
             FROM permissions p
             JOIN role_permissions rp ON rp.permission_id = p.id
             WHERE rp.role_id = $1
             ORDER BY p.code",
            role_id,
            "list_for_role",
        )
        .await
    }

    async fn list_for_user(&self, user_id: i32) -> Result<Vec<PermissionModel>, Error> {
        self.query_models(
            "SELECT p.id, p.code, p.name
             FROM users u
             JOIN roles r             ON r.id = u.role_id AND r.is_active
             JOIN role_permissions rp ON rp.role_id = r.id
             JOIN permissions p       ON p.id = rp.permission_id
             WHERE u.id = $1
             ORDER BY p.code",
            user_id,
            "list_for_user",
        )
        .await
    }

    async fn has_permission(&self, user_id: i32, key: &str) -> Result<bool, Error> {
        let db = self.db.as_ref();
        let row = ExistsRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT EXISTS (
                 SELECT 1
                 FROM users u
                 JOIN roles r             ON r.id = u.role_id AND r.is_active
                 JOIN role_permissions rp ON rp.role_id = r.id
                 JOIN permissions p       ON p.id = rp.permission_id
                 WHERE u.id = $1 AND p.code = $2
             ) AS granted",
            [user_id.into(), key.into()],
        ))
        .one(db)
        .await
        .map_err(|e| db_error("has_permission", e))?;

        Ok(row.is_some_and(|r| r.granted))
    }

    async fn find_actor(&self, user_id: i32) -> Result<Option<ActorRecord>, Error> {
        let db = self.db.as_ref();
        let Some(user) = ActorRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT u.id, u.username, r.code AS role_code,
                    (u.is_active AND u.deleted_at IS NULL) AS is_active
             FROM users u JOIN roles r ON r.id = u.role_id
             WHERE u.id = $1",
            [user_id.into()],
        ))
        .one(db)
        .await
        .map_err(|e| db_error("find_actor", e))?
        else {
            return Ok(None);
        };

        let permissions = self
            .list_for_user(user_id)
            .await?
            .into_iter()
            .map(|p| p.code)
            .collect();
        Ok(Some(ActorRecord {
            user_id: user.id,
            username: user.username,
            role_code: user.role_code,
            is_active: user.is_active,
            permissions,
        }))
    }
}
