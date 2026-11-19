use async_trait::async_trait;
use sea_orm::{
    sea_query::{Expr, OnConflict, Query},
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, QuerySelect, TransactionTrait,
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

use crate::entities::{
    permission,
    role::{self, Entity as Role},
    role_permission,
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
        // The ids of the permissions `keys` names.
        let named = Query::select()
            .column(permission::Column::Id)
            .from(permission::Entity)
            .and_where(permission::Column::Code.is_in(keys.iter().cloned()))
            .to_owned();
        // One transaction: a reader sees the old grants or the new ones,
        // never a role with half its permissions removed.
        let txn = self.db.begin().await.map_err(fail)?;
        let gone = Query::delete()
            .from_table(role_permission::Entity)
            .and_where(role_permission::Column::RoleId.eq(role_id))
            .and_where(role_permission::Column::PermissionId.not_in_subquery(named.clone()))
            .returning_col(role_permission::Column::PermissionId)
            .to_owned();
        let gone = GrantRow::find_by_statement(backend.build(&gone))
            .all(&txn)
            .await
            .map_err(fail)?;
        let new = Query::insert()
            .into_table(role_permission::Entity)
            .columns([
                role_permission::Column::RoleId,
                role_permission::Column::PermissionId,
                role_permission::Column::GrantedBy,
            ])
            .select_from(
                Query::select()
                    .expr(Expr::val(role_id))
                    .column(permission::Column::Id)
                    .expr(Expr::val(granted_by))
                    .from(permission::Entity)
                    .and_where(permission::Column::Code.is_in(keys.iter().cloned()))
                    .to_owned(),
            )
            .map_err(|e| Error::internal(format!("role grants (role {role_id}): {e}")))?
            .on_conflict(
                OnConflict::columns([
                    role_permission::Column::RoleId,
                    role_permission::Column::PermissionId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .returning_col(role_permission::Column::PermissionId)
            .to_owned();
        let new = GrantRow::find_by_statement(backend.build(&new))
            .all(&txn)
            .await
            .map_err(fail)?;
        let removed = codes_of(&txn, gone).await.map_err(fail)?;
        let added = codes_of(&txn, new).await.map_err(fail)?;
        txn.commit().await.map_err(fail)?;
        Ok(GrantChanges { added, removed })
    }
}

#[derive(FromQueryResult)]
struct GrantRow {
    permission_id: i32,
}

/// The codes of the permissions in `rows`, sorted.
async fn codes_of<C: ConnectionTrait>(
    db: &C,
    rows: Vec<GrantRow>,
) -> Result<Vec<String>, sea_orm::DbErr> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    permission::Entity::find()
        .select_only()
        .column(permission::Column::Code)
        .filter(permission::Column::Id.is_in(rows.into_iter().map(|r| r.permission_id)))
        .order_by_asc(permission::Column::Code)
        .into_tuple::<String>()
        .all(db)
        .await
}
