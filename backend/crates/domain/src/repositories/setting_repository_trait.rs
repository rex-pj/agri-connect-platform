use async_trait::async_trait;

use crate::{
    error::Error,
    models::setting::{SettingWrite, StoredSetting},
};

/// The settings moved off their defaults (`platform_settings`). Which keys
/// exist, their kinds and bounds is the application's registry; storage
/// keeps text.
#[async_trait]
pub trait SettingRepositoryTrait {
    /// Every stored setting, by key.
    async fn all(&self) -> Result<Vec<StoredSetting>, Error>;

    /// Applies `writes` in one transaction, stamping `actor_id`.
    async fn save(&self, writes: &[SettingWrite], actor_id: i32) -> Result<(), Error>;
}
