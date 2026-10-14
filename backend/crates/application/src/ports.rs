//! What the application needs from the outside world, without saying how.
//! Concrete adapters live in `infrastructure` and are chosen in `startup.rs`.

use async_trait::async_trait;
use domain::{models::role::UserRole, Result};

/// Send transactional emails. Delivery failures are logged by the adapter and
/// never surface to the caller: a password-reset request must look identical
/// whether or not the address exists.
#[async_trait]
pub trait EmailService: Send + Sync {
    async fn send_password_reset(&self, to_email: &str, token: &str);
}

/// What a verified session token says about its holder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionClaims {
    pub user_id:  i32,
    pub username: String,
    pub role:     UserRole,
    /// Expiry as a Unix timestamp in seconds.
    pub expires_at: i64,
}

/// Mint and verify signed session tokens.
pub trait TokenService: Send + Sync {
    fn mint(&self, user_id: i32, username: &str, role: &UserRole) -> Result<String>;

    /// Fails with `invalid_token` for a bad signature, a malformed token or an
    /// expired one; callers cannot tell those apart, by design.
    fn verify(&self, token: &str) -> Result<SessionClaims>;
}

/// Hash and verify passwords. Async because the adapter must run the CPU-heavy
/// work off the async worker threads.
#[async_trait]
pub trait PasswordHasher: Send + Sync {
    async fn hash(&self, password: &str) -> Result<String>;
    async fn verify(&self, password: &str, hash: &str) -> Result<bool>;
}
