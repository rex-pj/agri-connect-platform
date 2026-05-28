//! BCrypt implementation of the `PasswordHasher` application-layer port.
//!
//! CPU-intensive bcrypt operations are offloaded to Tokio's blocking thread
//! pool so Tokio worker threads are never stalled.

use async_trait::async_trait;
use tracing::error;

use crate::{application::ports::PasswordHasher, view_models::AppError};

pub struct BcryptPasswordHasher;

#[async_trait]
impl PasswordHasher for BcryptPasswordHasher {
    async fn hash(&self, password: &str) -> Result<String, AppError> {
        let password = password.to_owned();
        tokio::task::spawn_blocking(move || bcrypt::hash(&password, bcrypt::DEFAULT_COST))
            .await
            .map_err(|e| {
                error!("bcrypt hash join error: {e}");
                AppError::Internal("Password hashing error.".into())
            })?
            .map_err(|e| {
                error!("bcrypt hash error: {e}");
                AppError::Internal("Password hashing error.".into())
            })
    }

    async fn verify(&self, password: &str, hash: &str) -> Result<bool, AppError> {
        let password = password.to_owned();
        let hash     = hash.to_owned();
        tokio::task::spawn_blocking(move || bcrypt::verify(&password, &hash))
            .await
            .map_err(|e| {
                error!("bcrypt verify join error: {e}");
                AppError::Internal("Authentication error.".into())
            })?
            .map_err(|e| {
                error!("bcrypt verify error: {e}");
                AppError::Internal("Authentication error.".into())
            })
    }
}
