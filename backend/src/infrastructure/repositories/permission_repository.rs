use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    Set, Statement,
};
use std::sync::Arc;
use tracing::error;

use crate::{
    domain::{
        models::permission::PermissionModel,
        repositories::permission_repository_trait::PermissionRepositoryTrait,
    },
    entities::{
        permission::{self, Entity as Permission},
        user_permission::{self, Entity as UserPermission},
    },
    view_models::AppError,
};

pub struct PermissionRepository {
    db: Arc<DatabaseConnection>,
}

impl PermissionRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PermissionRepositoryTrait for PermissionRepository {
    async fn list_all(&self) -> Result<Vec<PermissionModel>, AppError> {
        Permission::find()
            .order_by_asc(permission::Column::Id)
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| PermissionModel { id: r.id, code: r.code, name: r.name })
                    .collect()
            })
            .map_err(|e| {
                error!("permission list_all error: {e}");
                AppError::Internal("Failed to load permissions.".into())
            })
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<PermissionModel>, AppError> {
        Permission::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(|r| PermissionModel { id: r.id, code: r.code, name: r.name }))
            .map_err(|e| {
                error!("permission find_by_id error: {e}");
                AppError::Internal("Failed to load permission.".into())
            })
    }

    async fn list_by_user(&self, user_id: i32) -> Result<Vec<PermissionModel>, AppError> {
        let db = self.db.as_ref();
        let sql = "
            SELECT ap.id, ap.code, ap.name
            FROM permissions ap
            JOIN user_permissions up ON up.permission_id = ap.id
            WHERE up.user_id = $1
            ORDER BY ap.id";

        db.query_all(Statement::from_sql_and_values(
            db.get_database_backend(),
            sql,
            [user_id.into()],
        ))
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|r| PermissionModel {
                    id:   r.try_get("", "id").unwrap_or_default(),
                    code: r.try_get("", "code").unwrap_or_default(),
                    name: r.try_get("", "name").unwrap_or_default(),
                })
                .collect()
        })
        .map_err(|e| {
            error!("permission list_by_user error: {e}");
            AppError::Internal("Failed to load user permissions.".into())
        })
    }

    async fn has_permission(&self, user_id: i32, code: &str) -> Result<bool, AppError> {
        let db = self.db.as_ref();
        let sql = "
            SELECT EXISTS(
                SELECT 1
                FROM user_permissions up
                JOIN permissions p ON p.id = up.permission_id
                WHERE up.user_id = $1 AND p.code = $2
            ) AS has_perm";

        db.query_one(Statement::from_sql_and_values(
            db.get_database_backend(),
            sql,
            [user_id.into(), code.into()],
        ))
        .await
        .map(|opt| {
            opt.map(|r| r.try_get::<bool>("", "has_perm").unwrap_or(false))
               .unwrap_or(false)
        })
        .map_err(|e| {
            error!("permission has_permission error: {e}");
            AppError::Internal("Permission check failed.".into())
        })
    }

    async fn assign_defaults_to_user(
        &self,
        user_id: i32,
        codes:   &[&str],
    ) -> Result<(), AppError> {
        if codes.is_empty() {
            return Ok(());
        }

        // Build a parameterised IN-list: $2, $3, …
        let placeholders = codes
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 2))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "INSERT INTO user_permissions (user_id, permission_id, granted_on)
             SELECT $1, id, NOW()
             FROM permissions
             WHERE code IN ({placeholders})
             ON CONFLICT (user_id, permission_id) DO NOTHING"
        );

        let mut values: Vec<sea_orm::Value> = vec![user_id.into()];
        for code in codes {
            values.push((*code).into());
        }

        self.db
            .as_ref()
            .execute(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                &sql,
                values,
            ))
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("permission assign_defaults_to_user error: {e}");
                AppError::Internal("Failed to assign default permissions.".into())
            })
    }

    async fn assign_to_user(
        &self,
        user_id:       i32,
        permission_id: i32,
        granted_by:    i32,
    ) -> Result<(), AppError> {
        let model = user_permission::ActiveModel {
            user_id:       Set(user_id),
            permission_id: Set(permission_id),
            granted_on:    Set(Utc::now().fixed_offset()),
            granted_by:    Set(Some(granted_by)),
        };

        UserPermission::insert(model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    user_permission::Column::UserId,
                    user_permission::Column::PermissionId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec_without_returning(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("permission assign_to_user error: {e}");
                AppError::Internal("Failed to assign permission.".into())
            })
    }

    async fn revoke_from_user(
        &self,
        user_id:       i32,
        permission_id: i32,
    ) -> Result<(), AppError> {
        UserPermission::delete_many()
            .filter(user_permission::Column::UserId.eq(user_id))
            .filter(user_permission::Column::PermissionId.eq(permission_id))
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("permission revoke_from_user error: {e}");
                AppError::Internal("Failed to revoke permission.".into())
            })
    }
}
