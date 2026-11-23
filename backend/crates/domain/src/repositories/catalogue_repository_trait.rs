use async_trait::async_trait;

use crate::{
    error::Error,
    models::catalogue::{Catalogue, PlatformCounts, SeedReport},
};

/// Applies the reference-data catalogue at startup.
///
/// Implementations must run everything in one transaction and be idempotent:
/// rows an admin changed or revoked must stay as the admin left them.
#[async_trait]
pub trait CatalogueRepositoryTrait {
    async fn apply(&self, catalogue: &Catalogue) -> Result<SeedReport, Error>;

    /// How much platform data there is (hidden rows included): what the demo
    /// data page lists as kept.
    async fn platform_counts(&self) -> Result<PlatformCounts, Error>;
}
