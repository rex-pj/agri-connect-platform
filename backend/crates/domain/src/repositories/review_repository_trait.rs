use async_trait::async_trait;

use crate::{error::Error, models::review::ReviewAdmin};

#[async_trait]
pub trait ReviewRepositoryTrait {
    /// `provider_id`: only that business's.
    async fn count_admin(&self, provider_id: Option<i32>) -> Result<i64, Error>;
    async fn list_admin(
        &self,
        provider_id: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ReviewAdmin>, Error>;
    async fn find_by_id_admin(&self, id: i32) -> Result<Option<ReviewAdmin>, Error>;

    async fn toggle_visible(&self, id: i32, is_visible: bool, admin_id: i32) -> Result<(), Error>;
}
