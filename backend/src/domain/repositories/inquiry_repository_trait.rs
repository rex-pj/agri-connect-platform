use async_trait::async_trait;

use crate::{
    domain::models::inquiry::{InquiryAdmin, NewInquiry},
    view_models::AppError,
};

#[async_trait]
pub trait InquiryRepositoryTrait {
    async fn create(&self, inquiry: NewInquiry) -> Result<(), AppError>;

    async fn count_admin(&self) -> Result<i64, AppError>;

    async fn list_admin(&self, limit: i64, offset: i64) -> Result<Vec<InquiryAdmin>, AppError>;

    async fn update_status(&self, id: i32, status: &str) -> Result<(), AppError>;
}
