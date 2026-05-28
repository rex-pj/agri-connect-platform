use std::sync::Arc;

use tracing::info;

use crate::{
    domain::repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        seed_runner_trait::SeedRunnerTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    view_models::{responses::SetupStatusDto, AppError},
};

#[derive(Clone)]
pub struct SetupUseCase {
    seed_runner:   Arc<dyn SeedRunnerTrait    + Send + Sync>,
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    user_repo:     Arc<dyn UserRepositoryTrait + Send + Sync>,
}

impl SetupUseCase {
    pub fn new(
        seed_runner:   Arc<dyn SeedRunnerTrait    + Send + Sync>,
        business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
        user_repo:     Arc<dyn UserRepositoryTrait + Send + Sync>,
    ) -> Self {
        Self { seed_runner, business_repo, user_repo }
    }

    pub async fn get_status(&self) -> Result<SetupStatusDto, AppError> {
        let (business_count, user_count) = tokio::try_join!(
            self.business_repo.count_all(),
            self.user_repo.count_all(),
        )?;

        Ok(SetupStatusDto {
            database_connected: true,
            is_seeded:          business_count > 0,
            business_count,
            user_count,
        })
    }

    pub async fn run_seed(&self) -> Result<SetupStatusDto, AppError> {
        if self.business_repo.count_all().await? > 0 {
            return Err(AppError::Conflict("Demo data has already been seeded.".into()));
        }

        self.seed_runner.run_seed().await?;

        info!("Demo seed completed via SeedMigrator");
        self.get_status().await
    }
}
