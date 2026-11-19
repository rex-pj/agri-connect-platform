use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};

use crate::{
    error::Error,
    models::demand::{BusinessArea, DemoScope, LeadCount, NewSearch, SearchCount, SoleCategory},
};

/// The reads behind the demand report: the leads and searches of a window,
/// counted by week, and how businesses map to areas and categories.
#[async_trait]
pub trait DemandRepositoryTrait {
    /// Leads in `[from, to)` of the businesses in `scope`, counted by week
    /// (whole weeks from `from`): call and Zalo taps, and inquiries that are
    /// not spam.
    async fn lead_counts(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        scope: DemoScope,
    ) -> Result<Vec<LeadCount>, Error>;

    /// Each positioned business's ward, as stored with its position;
    /// businesses with none are absent; only the businesses in `scope`.
    async fn business_areas(&self, scope: DemoScope) -> Result<Vec<BusinessArea>, Error>;

    /// Businesses with exactly one live service, with its category.
    async fn sole_categories(&self) -> Result<Vec<SoleCategory>, Error>;

    async fn record_search(&self, search: NewSearch) -> Result<(), Error>;

    /// Searches in `[from, to)` in `scope`, counted by week (whole weeks
    /// from `from`).
    async fn search_counts(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        scope: DemoScope,
    ) -> Result<Vec<SearchCount>, Error>;

    /// When the oldest tap or search before `cutoff` was made, if any.
    async fn oldest_event_before(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, Error>;

    /// Counts the taps and searches of `[from, to)`, the week starting on
    /// `week_starts_on`, into the weekly totals and deletes them, in one
    /// transaction; the rows deleted.
    async fn roll_up_week(
        &self,
        week_starts_on: NaiveDate,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<u64, Error>;
}
