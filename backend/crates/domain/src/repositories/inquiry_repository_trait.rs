use async_trait::async_trait;

use crate::{
    error::Error,
    models::{
        admin_list::AdminFilter,
        business::ListingRef,
        inquiry::{InquiryAdmin, InquiryForNotice, InquiryForOwner, NewInquiry},
    },
};

#[async_trait]
pub trait InquiryRepositoryTrait {
    async fn create(&self, inquiry: NewInquiry) -> Result<i32, Error>;

    /// The inquiry `id` with the email address of the provider it went to.
    async fn find_for_notice(&self, id: i32) -> Result<Option<InquiryForNotice>, Error>;

    /// How many requests `list_admin` pages through.
    async fn count_admin(&self, filter: &AdminFilter) -> Result<i64, Error>;

    /// Contact requests about one listing, spam left out.
    async fn count_for_listing(&self, listing: ListingRef) -> Result<i64, Error>;

    /// The newest inquiry from `phone` to the business that its provider
    /// answered (`INQUIRY_STATUSES_REVIEWABLE`): what lets that farmer review
    /// it. `None` when there is none.
    async fn find_reviewable(&self, business_id: i32, phone: &str) -> Result<Option<i32>, Error>;

    /// The admin list (`state`: one status; `search`: the sender's name
    /// or phone, or the provider's name).
    async fn list_admin(
        &self,
        filter: &AdminFilter,
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
