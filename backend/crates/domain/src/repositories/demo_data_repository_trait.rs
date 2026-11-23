use async_trait::async_trait;

use crate::{
    error::Error,
    models::catalogue::{DemoCounts, DemoRemoval, DemoReport, DemoVolume},
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

    /// Loads the demo data at volume: the file's, plus `volume` generated
    /// providers in the wards of the Mekong Delta with everything around
    /// them (a larger catalogue, accounts, listings, leads, reviews, prices,
    /// job posts, alerts, partners), each row as `load` writes it. The
    /// providers' and partners' accounts (flagged `is_demo`) all sign in
    /// with the password whose hash is `password_hash`.
    async fn load_volume(
        &self,
        volume: &DemoVolume,
        password_hash: &str,
    ) -> Result<DemoReport, Error>;

    /// Deletes every demo business (what belongs to one follows by cascade),
    /// the demo partners, prices, job posts, alerts, searches and accounts,
    /// then every demo commodity, type, area and crop no real row uses; the ones
    /// real data uses are kept and lose the flag. Returns the photos of the
    /// deleted services and products, still in storage.
    async fn remove(&self) -> Result<DemoRemoval, Error>;
}
