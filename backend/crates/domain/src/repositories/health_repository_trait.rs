use async_trait::async_trait;

use crate::error::Error;

#[async_trait]
pub trait HealthRepositoryTrait {
    /// Round-trips to the database. Fails when it cannot be reached.
    async fn ping(&self) -> Result<(), Error>;
}
