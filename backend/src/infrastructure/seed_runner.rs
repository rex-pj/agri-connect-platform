use std::sync::Arc;

use async_trait::async_trait;
use migration::{MigratorTrait, SeedMigrator};
use sea_orm::DatabaseConnection;

use crate::{
    domain::repositories::seed_runner_trait::SeedRunnerTrait,
    view_models::AppError,
};

pub struct SeedRunner {
    db: Arc<DatabaseConnection>,
}

impl SeedRunner {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SeedRunnerTrait for SeedRunner {
    async fn run_seed(&self) -> Result<(), AppError> {
        SeedMigrator::up(self.db.as_ref(), None)
            .await
            .map_err(|e| AppError::Internal(format!("Seed migration failed: {e}")))
    }
}
