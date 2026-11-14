//! Applies the platform catalogue (roles, permissions, default grants) in one
//! transaction, and counts the platform data.

use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use domain::{
    models::catalogue::{Catalogue, PlatformCounts, SeedReport},
    repositories::catalogue_repository_trait::CatalogueRepositoryTrait,
    Error, Result,
};
use sea_orm::{
    sea_query::{Expr, OnConflict, Query},
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait,
    IntoActiveModel, PaginatorTrait, QueryOrder, QuerySelect, Set, TransactionTrait,
};

use crate::{
    entities::{catalogue_state, permission, province, role, role_permission, ward},
    sql::advisory_xact_lock,
};

/// Recorded in `catalogue_state` once the full default-grant matrix has been
/// applied. Its presence is what stops later startups from re-granting
/// permissions an admin revoked.
const BOOTSTRAP_KEY: &str = "role_grants.bootstrap.v1";

/// Arbitrary but fixed: two processes starting together serialise on it, so
/// only one of them can apply the bootstrap grants.
const SEED_LOCK_ID: i64 = 0x6167_7269_5f63_6174; // "agri_cat"

pub struct CatalogueRepository {
    db: Arc<DatabaseConnection>,
}

impl CatalogueRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

fn db_error(step: &'static str) -> impl Fn(DbErr) -> Error {
    move |e| Error::internal(format!("catalogue {step}: {e}"))
}

/// Inserts `model` unless a row with the same `conflict` column exists;
/// the number of rows inserted (0 or 1).
async fn insert_new<A>(
    txn: &DatabaseTransaction,
    step: &'static str,
    model: A,
    conflict: <A::Entity as EntityTrait>::Column,
) -> Result<u64>
where
    A: ActiveModelTrait + Send,
    <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
{
    <A::Entity as EntityTrait>::insert(model)
        .on_conflict(OnConflict::column(conflict).do_nothing().to_owned())
        .exec_without_returning(txn)
        .await
        .map_err(db_error(step))
}

/// Grants permission `key` to role `role`, unless it already has it.
async fn grant(txn: &DatabaseTransaction, role: &str, key: &str) -> Result<u64> {
    let pair = Query::select()
        .column((role::Entity, role::Column::Id))
        .column((permission::Entity, permission::Column::Id))
        .from(role::Entity)
        .from(permission::Entity)
        .and_where(Expr::col((role::Entity, role::Column::Code)).eq(role))
        .and_where(Expr::col((permission::Entity, permission::Column::Code)).eq(key))
        .to_owned();
    let insert = Query::insert()
        .into_table(role_permission::Entity)
        .columns([
            role_permission::Column::RoleId,
            role_permission::Column::PermissionId,
        ])
        .select_from(pair)
        .map_err(|e| Error::internal(format!("catalogue grant: {e}")))?
        .on_conflict(OnConflict::new().do_nothing().to_owned())
        .to_owned();
    txn.execute(txn.get_database_backend().build(&insert))
        .await
        .map(|r| r.rows_affected())
        .map_err(db_error("grant"))
}

/// Number of rows in `entity`'s table.
async fn rows<E: EntityTrait, C: ConnectionTrait>(db: &C, step: &'static str) -> Result<u64>
where
    E::Model: Sync,
{
    E::find().count(db).await.map_err(db_error(step))
}

#[async_trait]
impl CatalogueRepositoryTrait for CatalogueRepository {
    async fn apply(&self, catalogue: &Catalogue) -> Result<SeedReport> {
        let txn = self.db.begin().await.map_err(db_error("begin"))?;
        txn.execute(
            txn.get_database_backend()
                .build(&advisory_xact_lock(SEED_LOCK_ID)),
        )
        .await
        .map_err(db_error("lock"))?;

        let mut report = SeedReport::default();

        for seed in catalogue.roles {
            report.roles_inserted += insert_new(
                &txn,
                "role",
                role::ActiveModel {
                    code: Set(seed.code.to_owned()),
                    name: Set(seed.code.to_owned()),
                    ..Default::default()
                },
                role::Column::Code,
            )
            .await?;
        }

        let mut new_keys = Vec::new();
        for perm in catalogue.permissions {
            let inserted = insert_new(
                &txn,
                "permission",
                permission::ActiveModel {
                    code: Set(perm.key.to_owned()),
                    name: Set(perm.key.to_owned()),
                    ..Default::default()
                },
                permission::Column::Code,
            )
            .await?;
            if inserted > 0 {
                new_keys.push(perm.key);
            }
        }
        report.permissions_inserted = new_keys.len() as u64;

        let bootstrapped = catalogue_state::Entity::find_by_id(BOOTSTRAP_KEY.to_owned())
            .count(&txn)
            .await
            .map_err(db_error("bootstrap check"))?
            > 0;

        for perm in catalogue.permissions {
            // After bootstrap only brand-new permissions get their defaults, so
            // an admin's revocation of an existing grant is never undone.
            if bootstrapped && !new_keys.contains(&perm.key) {
                continue;
            }
            for role in perm.default_roles {
                report.grants_inserted += grant(&txn, role, perm.key).await?;
            }
        }
        if !bootstrapped {
            catalogue_state::Entity::insert(catalogue_state::ActiveModel {
                key: Set(BOOTSTRAP_KEY.to_owned()),
                ..Default::default()
            })
            .exec_without_returning(&txn)
            .await
            .map_err(db_error("bootstrap mark"))?;
            report.bootstrapped_grants = true;
        }

        let known: HashSet<&str> = catalogue.permissions.iter().map(|p| p.key).collect();
        report.unknown_permissions = permission::Entity::find()
            .select_only()
            .column(permission::Column::Code)
            .order_by_asc(permission::Column::Code)
            .into_tuple::<String>()
            .all(&txn)
            .await
            .map_err(db_error("permission list"))?
            .into_iter()
            .filter(|code| !known.contains(code.as_str()))
            .collect();

        txn.commit().await.map_err(db_error("commit"))?;
        Ok(report)
    }

    async fn platform_counts(&self) -> Result<PlatformCounts> {
        let db = self.db.as_ref();
        let (roles, provinces, wards) = tokio::try_join!(
            rows::<role::Entity, _>(db, "role count"),
            rows::<province::Entity, _>(db, "province count"),
            rows::<ward::Entity, _>(db, "ward count"),
        )?;
        Ok(PlatformCounts {
            roles,
            provinces,
            wards,
        })
    }
}
