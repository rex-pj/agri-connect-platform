use std::sync::Arc;

use async_trait::async_trait;
use domain::{repositories::health_repository_trait::HealthRepositoryTrait, Error};
use sea_orm::DatabaseConnection;

pub struct HealthRepository {
    db: Arc<DatabaseConnection>,
}

impl HealthRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl HealthRepositoryTrait for HealthRepository {
    async fn ping(&self) -> Result<(), Error> {
        self.db
            .ping()
            .await
            .map_err(|e| Error::internal(format!("database ping: {e}")))
    }
}
