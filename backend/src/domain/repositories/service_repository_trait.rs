use async_trait::async_trait;

use crate::{
    domain::models::service::{AdminService, NewService, UpdateService},
    view_models::AppError,
};

#[async_trait]
pub trait ServiceRepositoryTrait {
    async fn list_admin(&self, provider_id: Option<i32>, limit: i64, offset: i64) -> Result<Vec<AdminService>, AppError>;

    async fn count_admin(&self, provider_id: Option<i32>) -> Result<i64, AppError>;

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<AdminService>, AppError>;

    async fn create(&self, service: NewService) -> Result<i32, AppError>;

    async fn update(&self, service: UpdateService) -> Result<bool, AppError>;

    async fn toggle_active(&self, id: i32, is_active: bool) -> Result<bool, AppError>;

    async fn delete(&self, id: i32) -> Result<bool, AppError>;

    async fn check_category_is_service(&self, category_id: i32) -> Result<bool, AppError>;
}
