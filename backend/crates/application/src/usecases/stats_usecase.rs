use std::sync::Arc;

use domain::{
    constants::{CALL_CHANNEL_PHONE, CALL_CHANNEL_ZALO},
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        call_repository_trait::CallRepositoryTrait,
        inquiry_repository_trait::InquiryRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Result,
};

use chrono::{DateTime, Duration, Utc};

use crate::{
    authz::Actor,
    catalogue::PERM_ADMIN_DASHBOARD_VIEW,
    constants::{CALL_STATS_DAYS, LEAD_WEEKS},
    outputs::{AdminStatsDto, WeekLeadsDto},
};

#[derive(Clone)]
pub struct StatsUseCase {
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    call_repo: Arc<dyn CallRepositoryTrait + Send + Sync>,
    inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
}

/// Everything `StatsUseCase` depends on, named so wiring cannot swap two
/// repos of compatible types by position.
pub struct StatsDeps {
    pub business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    pub product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    pub user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    pub call_repo: Arc<dyn CallRepositoryTrait + Send + Sync>,
    pub inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
}

impl StatsUseCase {
    pub fn new(d: StatsDeps) -> Self {
        Self {
            business_repo: d.business_repo,
            product_repo: d.product_repo,
            user_repo: d.user_repo,
            call_repo: d.call_repo,
            inquiry_repo: d.inquiry_repo,
        }
    }

    pub async fn get_stats(&self, actor: &Actor) -> Result<AdminStatsDto> {
        actor.require(PERM_ADMIN_DASHBOARD_VIEW)?;
        let now = Utc::now();
        let since = now - Duration::days(CALL_STATS_DAYS);
        let (total_businesses, verified_businesses, active_products, total_users, recent_calls) = tokio::try_join!(
            self.business_repo.count_all(),
            self.business_repo.count_verified(),
            self.product_repo.count_active(),
            self.user_repo.count_all(),
            self.call_repo.count_since(None, since),
        )?;
        let mut weeks = Vec::with_capacity(LEAD_WEEKS as usize);
        for back in 0..LEAD_WEEKS {
            let to = now - Duration::weeks(back);
            weeks.push(self.week(to - Duration::weeks(1), to).await?);
        }

        Ok(AdminStatsDto {
            total_businesses,
            verified_businesses,
            active_products,
            total_users,
            recent_calls,
            weeks,
        })
    }

    async fn week(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<WeekLeadsDto> {
        let (calls, zalo, inquiries) = tokio::try_join!(
            self.call_repo.count_window(CALL_CHANNEL_PHONE, from, to),
            self.call_repo.count_window(CALL_CHANNEL_ZALO, from, to),
            self.inquiry_repo.count_window(from, to),
        )?;
        Ok(WeekLeadsDto {
            from,
            to,
            calls,
            zalo,
            inquiries,
        })
    }
}
