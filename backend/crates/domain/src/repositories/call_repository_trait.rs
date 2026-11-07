use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{error::Error, models::call::NewCallClick};

#[async_trait]
pub trait CallRepositoryTrait {
    async fn record(&self, click: NewCallClick) -> Result<(), Error>;

    /// Taps since `since` through any channel: one business's, or every
    /// business's.
    async fn count_since(
        &self,
        business_id: Option<i32>,
        since: DateTime<Utc>,
    ) -> Result<i64, Error>;

    /// Every business's taps in `[from, to)` through `channel` (one of
    /// `CALL_CHANNELS`): the weekly figures of the admin dashboard.
    async fn count_window(
        &self,
        channel: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<i64, Error>;
}
