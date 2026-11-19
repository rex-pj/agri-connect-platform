use std::sync::Arc;

use domain::{
    repositories::health_repository_trait::HealthRepositoryTrait, Error, ErrorCode, Result,
};

#[derive(Clone)]
pub struct HealthUseCase {
    health_repo: Arc<dyn HealthRepositoryTrait + Send + Sync>,
}

impl HealthUseCase {
    pub fn new(health_repo: Arc<dyn HealthRepositoryTrait + Send + Sync>) -> Self {
        Self { health_repo }
    }

    /// Ready means the database answers. Nothing else is checked: a missing
    /// optional dependency (SMTP, storage) must not take the site out of the
    /// load balancer.
    pub async fn ready(&self) -> Result<()> {
        self.health_repo.ping().await.map_err(|e| {
            Error::new(ErrorCode::DatabaseUnavailable)
                .with_detail(e.detail().unwrap_or_default().to_owned())
        })
    }
}
