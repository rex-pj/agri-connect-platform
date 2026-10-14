use async_trait::async_trait;

use crate::{
    error::Error,
    models::catalogue::{Catalogue, SeedReport},
};

/// Applies the reference-data catalogue at startup.
///
/// Implementations must run everything in one transaction and be idempotent:
/// rows an admin changed or revoked must stay as the admin left them.
#[async_trait]
pub trait CatalogueRepositoryTrait {
    async fn apply(&self, catalogue: &Catalogue) -> Result<SeedReport, Error>;
}
