use std::sync::Arc;

use domain::{
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Result,
};

use crate::outputs::AdminStatsDto;

#[derive(Clone)]
pub struct StatsUseCase {
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
}

impl StatsUseCase {
    pub fn new(
        business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
        product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
        user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    ) -> Self {
        Self {
            business_repo,
            product_repo,
            user_repo,
        }
    }

    pub async fn get_stats(&self) -> Result<AdminStatsDto> {
        let (total_businesses, verified_businesses, active_products, total_users) = tokio::try_join!(
            self.business_repo.count_all(),
            self.business_repo.count_verified(),
            self.product_repo.count_active(),
            self.user_repo.count_all(),
        )?;

        Ok(AdminStatsDto {
            total_businesses,
            verified_businesses,
            active_products,
            total_users,
        })
    }
}
