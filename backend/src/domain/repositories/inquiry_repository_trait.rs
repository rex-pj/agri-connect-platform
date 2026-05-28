use async_trait::async_trait;

use crate::{
    domain::models::inquiry::{InquiryAdmin, NewInquiry},
    view_models::AppError,
};

#[async_trait]
pub trait InquiryRepositoryTrait {
    async fn create(&self, inquiry: NewInquiry) -> Result<i32, AppError>;

    async fn count_admin(&self) -> Result<i64, AppError>;

    async fn list_admin(&self, limit: i64, offset: i64) -> Result<Vec<InquiryAdmin>, AppError>;

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<InquiryAdmin>, AppError>;

    async fn update_status(&self, id: i32, status: &str, admin_id: i32) -> Result<(), AppError>;
}
