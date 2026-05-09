use std::sync::Arc;

use tracing::info;

use crate::{
    domain::repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        category_repository_trait::CategoryRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    view_models::{responses::SetupStatusDto, AppError},
};

#[derive(Clone)]
pub struct SetupUseCase {
    business_repo:  Arc<dyn BusinessRepositoryTrait  + Send + Sync>,
    category_repo:  Arc<dyn CategoryRepositoryTrait  + Send + Sync>,
    user_repo:      Arc<dyn UserRepositoryTrait       + Send + Sync>,
}

impl SetupUseCase {
    pub fn new(
        business_repo: Arc<dyn BusinessRepositoryTrait  + Send + Sync>,
        category_repo: Arc<dyn CategoryRepositoryTrait  + Send + Sync>,
        user_repo:     Arc<dyn UserRepositoryTrait       + Send + Sync>,
    ) -> Self {
        Self { business_repo, category_repo, user_repo }
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

    pub async fn verify_seed(&self) -> Result<SetupStatusDto, AppError> {
        let category_count = self.category_repo.count_active().await?;

        if category_count == 0 {
            return Err(AppError::Internal(
                "Danh mục chưa được khởi tạo. Hãy chạy migration trước.".into(),
            ));
        }

        info!("Setup check passed — {category_count} categories present");
        self.get_status().await
    }
}
