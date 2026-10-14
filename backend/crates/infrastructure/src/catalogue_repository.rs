//! Applies the reference catalogue (roles, permissions, default grants,
//! categories, location presets) in one transaction.

use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use domain::{
    models::catalogue::{Catalogue, SeedReport},
    repositories::catalogue_repository_trait::CatalogueRepositoryTrait,
    Error, Result,
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr, FromQueryResult, Statement,
    TransactionTrait, Value,
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

#[derive(FromQueryResult)]
struct CountRow {
    n: i64,
}

#[derive(FromQueryResult)]
struct CodeRow {
    code: String,
}

fn db_error(step: &str, e: DbErr) -> Error {
    Error::internal(format!("catalogue {step}: {e}"))
}

async fn exec(txn: &DatabaseTransaction, step: &str, sql: &str, values: Vec<Value>) -> Result<u64> {
    txn.execute(Statement::from_sql_and_values(txn.get_database_backend(), sql, values))
        .await
        .map(|r| r.rows_affected())
        .map_err(|e| db_error(step, e))
}

async fn count(txn: &DatabaseTransaction, step: &str, sql: &str, values: Vec<Value>) -> Result<i64> {
    CountRow::find_by_statement(Statement::from_sql_and_values(txn.get_database_backend(), sql, values))
        .one(txn)
        .await
        .map_err(|e| db_error(step, e))
        .map(|row| row.map_or(0, |r| r.n))
}

async fn grant(txn: &DatabaseTransaction, role: &str, key: &str) -> Result<u64> {
    exec(
        txn,
        "grant",
        "INSERT INTO role_permissions (role_id, permission_id)
         SELECT r.id, p.id FROM roles r, permissions p WHERE r.code = $1 AND p.code = $2
         ON CONFLICT DO NOTHING",
        vec![role.into(), key.into()],
    )
    .await
}

#[async_trait]
impl CatalogueRepositoryTrait for CatalogueRepository {
    async fn apply(&self, catalogue: &Catalogue) -> Result<SeedReport> {
        let txn = self.db.begin().await.map_err(|e| db_error("begin", e))?;
        exec(&txn, "lock", "SELECT pg_advisory_xact_lock($1)", vec![SEED_LOCK_ID.into()]).await?;

        let mut report = SeedReport::default();

        for role in catalogue.roles {
            report.roles_inserted += exec(
                &txn,
                "role",
                "INSERT INTO roles (code, name, description) VALUES ($1, $2, $3)
                 ON CONFLICT (code) DO NOTHING",
                vec![role.code.into(), role.name.into(), role.description.into()],
            )
            .await?;
        }

        let mut new_keys = Vec::new();
        for perm in catalogue.permissions {
            let inserted = exec(
                &txn,
                "permission",
                "INSERT INTO permissions (code, name) VALUES ($1, $2) ON CONFLICT (code) DO NOTHING",
                vec![perm.key.into(), perm.name.into()],
            )
            .await?;
            if inserted > 0 {
                new_keys.push(perm.key);
            }
        }
        report.permissions_inserted = new_keys.len() as u64;

        let bootstrapped = count(
            &txn,
            "bootstrap check",
            "SELECT COUNT(*) AS n FROM catalogue_state WHERE key = $1",
            vec![BOOTSTRAP_KEY.into()],
        )
        .await?
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
            exec(
                &txn,
                "bootstrap mark",
                "INSERT INTO catalogue_state (key) VALUES ($1)",
                vec![BOOTSTRAP_KEY.into()],
            )
            .await?;
            report.bootstrapped_grants = true;
        }

        let known: HashSet<&str> = catalogue.permissions.iter().map(|p| p.key).collect();
        report.unknown_permissions = CodeRow::find_by_statement(Statement::from_string(
            txn.get_database_backend(),
            "SELECT code FROM permissions ORDER BY code",
        ))
        .all(&txn)
        .await
        .map_err(|e| db_error("permission list", e))?
        .into_iter()
        .map(|r| r.code)
        .filter(|code| !known.contains(code.as_str()))
        .collect();

        // Categories and presets are admin-editable: seed an empty table only,
        // or a renamed slug would come back as a duplicate at the next boot.
        if count(&txn, "category count", "SELECT COUNT(*) AS n FROM categories", vec![]).await? == 0 {
            for c in catalogue.categories {
                report.categories_inserted += exec(
                    &txn,
                    "category",
                    "INSERT INTO categories (name, slug, type, icon, color, description, display_order)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)",
                    vec![
                        c.name.into(), c.slug.into(), c.kind.into(), c.icon.into(),
                        c.color.into(), c.description.into(), c.display_order.into(),
                    ],
                )
                .await?;
            }
        }

        if count(&txn, "location count", "SELECT COUNT(*) AS n FROM locations", vec![]).await? == 0 {
            for l in catalogue.locations {
                report.locations_inserted += exec(
                    &txn,
                    "location",
                    "INSERT INTO locations (label, lat, lng, display_order) VALUES ($1, $2, $3, $4)",
                    vec![l.label.into(), l.lat.into(), l.lng.into(), l.display_order.into()],
                )
                .await?;
            }
        }

        txn.commit().await.map_err(|e| db_error("commit", e))?;
        Ok(report)
    }
}
