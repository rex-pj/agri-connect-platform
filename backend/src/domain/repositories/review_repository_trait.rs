use async_trait::async_trait;

use crate::{domain::models::review::ReviewAdmin, view_models::AppError};

#[async_trait]
pub trait ReviewRepositoryTrait {
    async fn count_admin(&self) -> Result<i64, AppError>;
    async fn list_admin(&self, limit: i64, offset: i64) -> Result<Vec<ReviewAdmin>, AppError>;
    async fn find_by_id_admin(&self, id: i32) -> Result<Option<ReviewAdmin>, AppError>;

    async fn toggle_visible(&self, id: i32, is_visible: bool, admin_id: i32) -> Result<(), AppError>;
}
