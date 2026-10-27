use std::sync::Arc;

use domain::{
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        call_repository_trait::CallRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Result,
};

use chrono::{Duration, Utc};

use crate::{
    authz::Actor, catalogue::PERM_ADMIN_DASHBOARD_VIEW, constants::CALL_STATS_DAYS,
    outputs::AdminStatsDto,
};

#[derive(Clone)]
pub struct StatsUseCase {
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    call_repo: Arc<dyn CallRepositoryTrait + Send + Sync>,
}

impl StatsUseCase {
    pub fn new(
        business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
        product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
        user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
        call_repo: Arc<dyn CallRepositoryTrait + Send + Sync>,
    ) -> Self {
        Self {
            business_repo,
            product_repo,
            user_repo,
            call_repo,
        }
    }

    pub async fn get_stats(&self, actor: &Actor) -> Result<AdminStatsDto> {
        actor.require(PERM_ADMIN_DASHBOARD_VIEW)?;
        let since = Utc::now() - Duration::days(CALL_STATS_DAYS);
        let (total_businesses, verified_businesses, active_products, total_users, recent_calls) = tokio::try_join!(
            self.business_repo.count_all(),
            self.business_repo.count_verified(),
            self.product_repo.count_active(),
            self.user_repo.count_all(),
            self.call_repo.count_since(None, since),
        )?;

        Ok(AdminStatsDto {
            total_businesses,
            verified_businesses,
            active_products,
            total_users,
            recent_calls,
        })
    }
}
