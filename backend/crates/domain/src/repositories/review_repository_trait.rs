use async_trait::async_trait;

use crate::{
    error::Error,
    models::admin_list::AdminFilter,
    models::review::{
        BusinessRating, NewReview, OwnReview, PublicReview, RatingSummary, ReviewAdmin,
    },
};

#[async_trait]
pub trait ReviewRepositoryTrait {
    /// How many reviews `list_admin` pages through.
    async fn count_admin(&self, filter: &AdminFilter) -> Result<i64, Error>;
    /// The admin list (`state`: `waiting` / `shown`; `search`: the
    /// reviewer's name, the comment or the provider's name).
    async fn list_admin(
        &self,
        filter: &AdminFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ReviewAdmin>, Error>;
    async fn find_by_id_admin(&self, id: i32) -> Result<Option<ReviewAdmin>, Error>;

    async fn toggle_visible(&self, id: i32, is_visible: bool, admin_id: i32) -> Result<(), Error>;

    /// Stores a review, hidden; its id. A second one from the same phone for
    /// the same business is the field error `review_already_sent`.
    async fn create(&self, review: NewReview) -> Result<i32, Error>;

    /// The business's shown reviews, newest first.
    async fn list_public(&self, business_id: i32, limit: i64) -> Result<Vec<PublicReview>, Error>;

    async fn summary_public(&self, business_id: i32) -> Result<RatingSummary, Error>;

    /// The shown reviews of each of these businesses, in one query; a
    /// business with none is absent.
    async fn ratings_public(&self, business_ids: &[i32]) -> Result<Vec<BusinessRating>, Error>;

    /// A page of the reviews of the business `owner_user` manages, newest
    /// first.
    async fn list_for_owner(
        &self,
        owner_user: i32,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<OwnReview>, Error>;

    /// How many reviews the business `owner_user` manages has, and how many
    /// of them wait to be shown.
    async fn count_for_owner(&self, owner_user: i32) -> Result<(i64, i64), Error>;

    /// Sets (or with `None` removes) the answer to a review of the business
    /// `owner_user` manages, in one statement; false when it is not theirs.
    async fn set_reply_for_owner(
        &self,
        owner_user: i32,
        id: i32,
        reply: Option<String>,
    ) -> Result<bool, Error>;
}
