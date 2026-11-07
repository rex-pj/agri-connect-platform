use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    error::Error,
    models::demand::{BusinessArea, LeadEvent, NewSearch, SearchEvent, SoleCategory},
};

/// The reads behind the demand report: every lead in a window, and how
/// businesses map to areas and categories.
#[async_trait]
pub trait DemandRepositoryTrait {
    /// Leads in `[from, to)` of businesses that are not demo data: call and
    /// Zalo taps, and inquiries that are not spam.
    async fn lead_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<LeadEvent>, Error>;

    /// Each positioned business's nearest active location preset within
    /// `radius_m` metres; businesses with none in reach are absent.
    async fn business_areas(&self, radius_m: f64) -> Result<Vec<BusinessArea>, Error>;

    /// Businesses with exactly one live service, with its category.
    async fn sole_categories(&self) -> Result<Vec<SoleCategory>, Error>;

    /// The nearest active location preset to a point within `radius_m`.
    async fn nearest_location(
        &self,
        lat: f64,
        lng: f64,
        radius_m: f64,
    ) -> Result<Option<i32>, Error>;

    async fn record_search(&self, search: NewSearch) -> Result<(), Error>;

    /// Searches in `[from, to)`.
    async fn search_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<SearchEvent>, Error>;
}
