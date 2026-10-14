//! BCrypt implementation of the `PasswordHasher` application-layer port.
//!
//! CPU-intensive bcrypt operations are offloaded to Tokio's blocking thread
//! pool so Tokio worker threads are never stalled.

use async_trait::async_trait;
use tracing::error;

use application::ports::PasswordHasher;
use domain::Error;

pub struct BcryptPasswordHasher;

#[async_trait]
impl PasswordHasher for BcryptPasswordHasher {
    async fn hash(&self, password: &str) -> Result<String, Error> {
        let password = password.to_owned();
        tokio::task::spawn_blocking(move || bcrypt::hash(&password, bcrypt::DEFAULT_COST))
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
        let hash     = hash.to_owned();
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
