use async_trait::async_trait;

use crate::{
    error::Error,
    models::inquiry::{InquiryAdmin, NewInquiry},
};

#[async_trait]
pub trait InquiryRepositoryTrait {
    async fn create(&self, inquiry: NewInquiry) -> Result<i32, Error>;

    async fn count_admin(&self) -> Result<i64, Error>;

    async fn list_admin(&self, limit: i64, offset: i64) -> Result<Vec<InquiryAdmin>, Error>;

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<InquiryAdmin>, Error>;

    async fn update_status(&self, id: i32, status: &str, admin_id: i32) -> Result<(), Error>;
}
