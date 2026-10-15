//! Refresh tokens as the domain sees them. The raw token never appears here:
//! only its hash is stored and looked up.

use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshToken {
    pub id: Uuid,
    /// Shared by every token rotated from the same login.
    pub family_id: Uuid,
    pub user_id: i32,
    pub expires_at: DateTime<Utc>,
    /// Set when the token was exchanged for a new one.
    pub used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRefreshToken {
    pub id: Uuid,
    pub family_id: Uuid,
    pub user_id: i32,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}
