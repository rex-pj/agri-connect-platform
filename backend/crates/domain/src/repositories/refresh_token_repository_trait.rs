use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    error::Error,
    models::session::{NewRefreshToken, RefreshToken},
};

#[async_trait]
pub trait RefreshTokenRepositoryTrait {
    async fn insert(&self, token: &NewRefreshToken) -> Result<(), Error>;

    async fn find_by_hash(&self, token_hash: &str) -> Result<Option<RefreshToken>, Error>;

    /// Marks the token used if it is still unused and not revoked. `false`
    /// when another request got there first: exactly one rotation wins.
    async fn claim(&self, id: Uuid) -> Result<bool, Error>;

    async fn revoke_family(&self, family_id: Uuid) -> Result<(), Error>;

    /// Whether the session `family_id` still stands: it has a token that was
    /// not revoked (logout, a password change, a deactivation, a reuse).
    async fn family_live(&self, family_id: Uuid) -> Result<bool, Error>;

    /// Every session of the user, e.g. after a password reset.
    async fn revoke_user(&self, user_id: i32) -> Result<(), Error>;

    /// Removes tokens that expired before `before`; returns how many.
    async fn delete_expired(&self, before: DateTime<Utc>) -> Result<u64, Error>;
}
