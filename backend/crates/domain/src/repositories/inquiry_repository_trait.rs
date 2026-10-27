use async_trait::async_trait;

use crate::{
    error::Error,
    models::inquiry::{InquiryAdmin, InquiryForNotice, InquiryForOwner, NewInquiry},
};

#[async_trait]
pub trait InquiryRepositoryTrait {
    async fn create(&self, inquiry: NewInquiry) -> Result<i32, Error>;

    /// The inquiry `id` with the email address of the provider it went to.
    async fn find_for_notice(&self, id: i32) -> Result<Option<InquiryForNotice>, Error>;

    /// `provider_id`: only that business's.
    async fn count_admin(&self, provider_id: Option<i32>) -> Result<i64, Error>;

    async fn list_admin(
        &self,
        provider_id: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<InquiryAdmin>, Error>;

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<InquiryAdmin>, Error>;

    async fn update_status(&self, id: i32, status: &str, admin_id: i32) -> Result<(), Error>;

    // ── The provider's own inbox: inquiries to the business `owner_user` owns ──

    /// (all, still `new`).
    async fn count_for_owner(&self, owner_user: i32) -> Result<(i64, i64), Error>;

    /// Newest first.
    async fn list_for_owner(
        &self,
        owner_user: i32,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<InquiryForOwner>, Error>;

    /// False when the inquiry is not one sent to their business.
    async fn update_status_for_owner(
        &self,
        id: i32,
        owner_user: i32,
        status: &str,
    ) -> Result<bool, Error>;
}
