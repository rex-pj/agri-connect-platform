use async_trait::async_trait;

use crate::{
    error::Error,
    models::catalogue::{DemoCounts, DemoRemoval, DemoReport},
};

/// Demo data for trying the platform out: a starting catalogue (commodities,
/// service and supply types, area presets, crops) and listings in it. Every commodity, type, area, crop and
/// business it writes is flagged `is_demo`, and everything else it writes
/// belongs to one of those businesses, so `remove` never touches a real row
/// nor the platform data (roles, permissions, provinces, wards).
#[async_trait]
pub trait DemoDataRepositoryTrait {
    /// The demo rows present now.
    async fn count(&self) -> Result<DemoCounts, Error>;

    /// Loads the demo catalogue and listings. A demo commodity, type, area or
    /// crop already present (same name, slug, label or name) is reused.
    async fn load(&self) -> Result<DemoReport, Error>;

    /// Deletes every demo business (what belongs to one follows by cascade),
    /// then every demo commodity, type, area and crop no real row uses; the ones
    /// real data uses are kept and lose the flag. Returns the photos of the
    /// deleted services and products, still in storage.
    async fn remove(&self) -> Result<DemoRemoval, Error>;
}
