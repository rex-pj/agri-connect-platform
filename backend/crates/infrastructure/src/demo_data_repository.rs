//! Sample listings for testing, loaded only on request.
//!
//! Services and products join on category slugs from the reference catalogue;
//! a slug an admin renamed makes its demo rows silently drop out, and the
//! returned counts show it.

use std::sync::Arc;

use async_trait::async_trait;
use domain::{
    models::catalogue::DemoReport,
    repositories::demo_data_repository_trait::DemoDataRepositoryTrait, Error, Result,
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbErr, FromQueryResult, Statement, TransactionTrait,
};

const BUSINESSES_SQL: &str = include_str!("demo/businesses.sql");
const SERVICES_SQL: &str = include_str!("demo/services.sql");
const PRODUCTS_SQL: &str = include_str!("demo/products.sql");

pub struct DemoDataRepository {
    db: Arc<DatabaseConnection>,
}

impl DemoDataRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[derive(FromQueryResult)]
struct CountRow {
    n: i64,
}

fn db_error(step: &str, e: DbErr) -> Error {
    Error::internal(format!("demo data {step}: {e}"))
}

#[async_trait]
impl DemoDataRepositoryTrait for DemoDataRepository {
    async fn count(&self) -> Result<i64> {
        let db = self.db.as_ref();
        CountRow::find_by_statement(Statement::from_string(
            db.get_database_backend(),
            "SELECT COUNT(*) AS n FROM businesses WHERE is_demo",
        ))
        .one(db)
        .await
        .map_err(|e| db_error("count", e))
        .map(|row| row.map_or(0, |r| r.n))
    }

    async fn load(&self) -> Result<DemoReport> {
        // All or nothing: half-loaded demo data would make `count` say "loaded"
        // while services or products are missing.
        let txn = self.db.begin().await.map_err(|e| db_error("begin", e))?;
        let backend = txn.get_database_backend();
        let run = |sql: &'static str, step: &'static str| {
            let txn = &txn;
            async move {
                txn.execute(Statement::from_string(backend, sql))
                    .await
                    .map(|r| r.rows_affected())
                    .map_err(|e| db_error(step, e))
            }
        };
        let businesses = run(BUSINESSES_SQL, "businesses").await?;
        let services = run(SERVICES_SQL, "services").await?;
        let products = run(PRODUCTS_SQL, "products").await?;
        txn.commit().await.map_err(|e| db_error("commit", e))?;
        Ok(DemoReport { businesses, services, products })
    }

    async fn remove(&self) -> Result<u64> {
        let db = self.db.as_ref();
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "DELETE FROM businesses WHERE is_demo",
        ))
        .await
        .map(|r| r.rows_affected())
        .map_err(|e| db_error("remove", e))
    }
}
