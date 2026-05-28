//! Application-layer port traits.
//!
//! These traits define *what* the application layer needs from the outside
//! world without committing to *how* those needs are met. Concrete
//! implementations live in `infrastructure/`.
//!
//! Dependency direction:
//!   Infrastructure  →  Application (ports)  ←  Use cases
//!
//! Use cases depend only on these traits — never on concrete library types.

use async_trait::async_trait;

use crate::{domain::models::role::UserRole, view_models::AppError};

// ── EmailService ──────────────────────────────────────────────────────────────

/// Send transactional emails.
///
/// The concrete implementation uses lettre/SMTP; the use case never sees that.
#[async_trait]
pub trait EmailService: Send + Sync {
    async fn send_password_reset(&self, to_email: &str, token: &str);
}

// ── TokenService ──────────────────────────────────────────────────────────────

/// Mint a signed session token for an authenticated user.
///
/// The concrete implementation in `infrastructure/` uses JWT; the use case
/// never needs to know that.
pub trait TokenService: Send + Sync {
    fn mint(&self, user_id: i32, username: &str, role: &UserRole) -> Result<String, AppError>;
}

// ── PasswordHasher ────────────────────────────────────────────────────────────

/// Hash and verify user passwords.
///
/// Operations are `async` because the concrete bcrypt implementation must
/// be offloaded to a blocking thread pool. The use case sees only this
/// abstract interface.
#[async_trait]
pub trait PasswordHasher: Send + Sync {
    async fn hash(&self, password: &str) -> Result<String, AppError>;
    async fn verify(&self, password: &str, hash: &str) -> Result<bool, AppError>;
}
