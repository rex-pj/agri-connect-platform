use async_trait::async_trait;

use crate::view_models::AppError;

#[async_trait]
pub trait SeedRunnerTrait {
    async fn run_seed(&self) -> Result<(), AppError>;
}
