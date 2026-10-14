//! Session extraction. Reads and verifies the bearer token through the
//! `TokenService` port; no JWT types appear here.
//!
//! `RequireAdmin` / `RequireProviderOrAdmin` still decide access by role.
//! Slice S2 replaces them with permission checks inside the use cases.

use axum::{extract::FromRequestParts, http::request::Parts};
use domain::{models::role::UserRole, Error, ErrorCode};

use crate::app_state::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i32,
    pub role: UserRole,
}

fn bearer_token(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token =
            bearer_token(parts).ok_or_else(|| Error::new(ErrorCode::AuthenticationRequired))?;
        let claims = state.token_service.verify(token)?;
        Ok(AuthUser {
            id: claims.user_id,
            role: claims.role,
        })
    }
}

/// Admits `UserRole::Admin` only.
pub struct RequireAdmin(pub AuthUser);

impl FromRequestParts<AppState> for RequireAdmin {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if user.role != UserRole::Admin {
            return Err(Error::new(ErrorCode::PermissionDenied));
        }
        Ok(RequireAdmin(user))
    }
}

/// Admits `UserRole::Admin` and `UserRole::Provider`.
pub struct RequireProviderOrAdmin(pub AuthUser);

impl FromRequestParts<AppState> for RequireProviderOrAdmin {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if !matches!(user.role, UserRole::Admin | UserRole::Provider) {
            return Err(Error::new(ErrorCode::PermissionDenied));
        }
        Ok(RequireProviderOrAdmin(user))
    }
}
