use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{error::Error, models::call::NewCallClick};

#[async_trait]
pub trait CallRepositoryTrait {
    async fn record(&self, click: NewCallClick) -> Result<(), Error>;

    /// Taps since `since` through any channel on real (not demo) shops: one
    /// business's, or every
    /// business's.
    async fn count_since(
        &self,
        business_id: Option<i32>,
        since: DateTime<Utc>,
    ) -> Result<i64, Error>;
}
