//! BCrypt implementation of the `PasswordHasher` application-layer port.
//!
//! CPU-intensive bcrypt operations are offloaded to Tokio's blocking thread
//! pool so Tokio worker threads are never stalled.

use async_trait::async_trait;
use tracing::error;

use application::ports::PasswordHasher;
use domain::Error;

/// Hashes at `cost` (2^cost rounds). Verifying reads the cost from the stored
/// hash, so hashes made at any cost keep verifying when it changes.
pub struct BcryptPasswordHasher {
    cost: u32,
}

impl BcryptPasswordHasher {
    /// The cheapest cost bcrypt accepts. Only for tests, where thousands of
    /// hashes would otherwise spend minutes doing what no test checks.
    pub const MIN_COST: u32 = 4;

    pub fn with_cost(cost: u32) -> Self {
        Self { cost }
    }
}

/// bcrypt's recommended cost: what production uses.
impl Default for BcryptPasswordHasher {
    fn default() -> Self {
        Self::with_cost(bcrypt::DEFAULT_COST)
    }
}

#[async_trait]
impl PasswordHasher for BcryptPasswordHasher {
    async fn hash(&self, password: &str) -> Result<String, Error> {
        let password = password.to_owned();
        let cost = self.cost;
        tokio::task::spawn_blocking(move || bcrypt::hash(&password, cost))
            .await
            .map_err(|e| {
                error!("bcrypt hash join error: {e}");
                Error::internal("Password hashing error.")
            })?
            .map_err(|e| {
                error!("bcrypt hash error: {e}");
                Error::internal("Password hashing error.")
            })
    }

    async fn verify(&self, password: &str, hash: &str) -> Result<bool, Error> {
        let password = password.to_owned();
        let hash = hash.to_owned();
        tokio::task::spawn_blocking(move || bcrypt::verify(&password, &hash))
            .await
            .map_err(|e| {
                error!("bcrypt verify join error: {e}");
                Error::internal("Authentication error.")
            })?
            .map_err(|e| {
                error!("bcrypt verify error: {e}");
                Error::internal("Authentication error.")
            })
    }
}
