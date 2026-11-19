use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    error::Error,
    models::{business::ListingRef, call::NewCallClick},
};

#[async_trait]
pub trait CallRepositoryTrait {
    async fn record(&self, click: NewCallClick) -> Result<(), Error>;

    /// Taps since `since` through any channel: one business's, or every
    /// business's (the admin's total, demo shops included while demo data
    /// is loaded, like the demand report).
    async fn count_since(
        &self,
        business_id: Option<i32>,
        since: DateTime<Utc>,
    ) -> Result<i64, Error>;

    /// Taps since `since` on one listing's buttons (a service's, a supply's).
    async fn count_listing_since(
        &self,
        listing: ListingRef,
        since: DateTime<Utc>,
    ) -> Result<i64, Error>;
}
