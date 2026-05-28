//! JWT implementation of the `TokenService` application-layer port.
//!
//! This module is the only place in the codebase where `jsonwebtoken` types
//! are used for token minting. The application layer (use cases) depends
//! only on the `TokenService` trait — never on JWT internals.

use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Header};
use tracing::error;

use crate::{
    application::ports::TokenService,
    constants::JWT_EXPIRY_HOURS,
    domain::models::role::UserRole,
    middleware::auth::{encoding_key, Claims},
    view_models::AppError,
};

pub struct JwtTokenService;

impl TokenService for JwtTokenService {
    fn mint(&self, user_id: i32, username: &str, role: &UserRole) -> Result<String, AppError> {
        let exp = (Utc::now() + Duration::hours(JWT_EXPIRY_HOURS)).timestamp() as usize;
        let claims = Claims {
            sub:      user_id,
            role:     role.clone(),
            username: username.to_owned(),
            exp,
        };
        encode(&Header::default(), &claims, encoding_key()).map_err(|e| {
            error!("JWT encode error: {e}");
            AppError::Internal("Failed to create session.".into())
        })
    }
}
