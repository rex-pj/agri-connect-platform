use async_trait::async_trait;

use crate::{
    error::Error,
    models::catalogue::{DemoRemoval, DemoReport},
};

/// Sample listings for testing. Every row it writes is flagged `is_demo`, so
/// `remove` can never touch a real listing.
#[async_trait]
pub trait DemoDataRepositoryTrait {
    /// Number of demo businesses currently present.
    async fn count(&self) -> Result<i64, Error>;

    async fn load(&self) -> Result<DemoReport, Error>;

    /// Deletes every demo business; services and products follow by cascade.
    /// Returns the photos of the deleted products, still in storage.
    async fn remove(&self) -> Result<DemoRemoval, Error>;
}
