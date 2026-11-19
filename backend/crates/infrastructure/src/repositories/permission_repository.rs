use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use domain::{
    models::{permission::PermissionModel, user::ActorRecord},
    repositories::permission_repository_trait::PermissionRepositoryTrait,
    Error,
};
use sea_orm::{
    sea_query::{Expr, Query, SelectStatement},
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, FromQueryResult, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait,
};

use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        permission::{self, Entity as Permission},
        role, role_permission, user,
    },
    sql::QueryAs,
};

pub struct PermissionRepository {
    db: Arc<DatabaseConnection>,
}

impl PermissionRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// The permissions whose id `ids` selects, by code.
    async fn permissions_in(
        &self,
        ids: SelectStatement,
        what: &'static str,
    ) -> Result<Vec<PermissionModel>, Error> {
        Permission::find()
            .filter(permission::Column::Id.in_subquery(ids))
            .order_by_asc(permission::Column::Code)
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_model).collect())
            .map_err(|e| db_error(what, e))
    }
}

#[derive(FromQueryResult)]
struct ActorRow {
    id: i32,
    username: String,
    role_code: String,
    is_active: bool,
    sessions_valid_after: Option<DateTimeWithTimeZone>,
}

fn to_model(r: permission::Model) -> PermissionModel {
    PermissionModel {
        id: r.id,
        code: r.code,
        name: r.name,
    }
}

fn db_error(what: &str, e: DbErr) -> Error {
    Error::internal(format!("permission {what}: {e}"))
}

/// The ids of the permissions granted to `role_id`.
fn granted_to_role(role_id: i32) -> SelectStatement {
    Query::select()
        .column(role_permission::Column::PermissionId)
        .from(role_permission::Entity)
        .and_where(role_permission::Column::RoleId.eq(role_id))
        .to_owned()
}

/// The ids of the permissions `user_id` holds through its role, while that
/// role is active.
fn granted_to_user(user_id: i32) -> SelectStatement {
    Query::select()
        .column((
            role_permission::Entity,
            role_permission::Column::PermissionId,
        ))
        .from(role_permission::Entity)
        .inner_join(
            role::Entity,
            Expr::col((role::Entity, role::Column::Id))
                .equals((role_permission::Entity, role_permission::Column::RoleId))
                .and(Expr::col((role::Entity, role::Column::IsActive)).eq(true)),
        )
        .inner_join(
            user::Entity,
            Expr::col((user::Entity, user::Column::RoleId))
                .equals((role::Entity, role::Column::Id)),
        )
        .and_where(Expr::col((user::Entity, user::Column::Id)).eq(user_id))
        .to_owned()
}

#[async_trait]
impl PermissionRepositoryTrait for PermissionRepository {
    async fn list_all(&self) -> Result<Vec<PermissionModel>, Error> {
        Permission::find()
            .order_by_asc(permission::Column::Code)
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_model).collect())
            .map_err(|e| db_error("list_all", e))
    }

    async fn list_for_role(&self, role_id: i32) -> Result<Vec<PermissionModel>, Error> {
        self.permissions_in(granted_to_role(role_id), "list_for_role")
            .await
    }

    async fn list_for_user(&self, user_id: i32) -> Result<Vec<PermissionModel>, Error> {
        self.permissions_in(granted_to_user(user_id), "list_for_user")
            .await
    }

    async fn has_permission(&self, user_id: i32, key: &str) -> Result<bool, Error> {
        Permission::find()
            .filter(permission::Column::Id.in_subquery(granted_to_user(user_id)))
            .filter(permission::Column::Code.eq(key))
            .count(self.db.as_ref())
            .await
            .map(|n| n > 0)
            .map_err(|e| db_error("has_permission", e))
    }

    async fn find_actor(&self, user_id: i32) -> Result<Option<ActorRecord>, Error> {
        let Some(user) = user::Entity::find_by_id(user_id)
            .select_only()
            .column(user::Column::Id)
            .column(user::Column::Username)
            .column_as(role::Column::Code, QueryAs::RoleCode)
            // A deleted account is an inactive one.
            .expr_as(
                Expr::col((user::Entity, user::Column::IsActive))
                    .eq(true)
                    .and(Expr::col((user::Entity, user::Column::DeletedAt)).is_null()),
                user::Column::IsActive,
            )
            .column(user::Column::SessionsValidAfter)
            .join(JoinType::InnerJoin, user::Relation::Role.def())
            .into_model::<ActorRow>()
            .one(self.db.as_ref())
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
            sessions_valid_after: user.sessions_valid_after.map(|t| t.with_timezone(&Utc)),
        }))
    }
}
