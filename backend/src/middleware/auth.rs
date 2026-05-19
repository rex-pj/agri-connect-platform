//! JWT-based authentication and role guards for Axum.
//!
//! # Roles
//!
//! | Role       | Who                          | DB value   |
//! |------------|------------------------------|------------|
//! | Admin      | Platform staff               | `"admin"`  |
//! | Provider   | Business owners              | `"provider"`|
//! | Guest      | Unauthenticated visitors     | *(no token)* |
//!
//! # Usage in a handler
//!
//! ```rust
//! // Accessible only by admins:
//! pub async fn admin_only(RequireAdmin(user): RequireAdmin) -> impl IntoResponse { ... }
//!
//! // Accessible by admins and providers:
//! pub async fn provider_action(RequireProvider(user): RequireProvider, ...) -> impl IntoResponse { ... }
//!
//! // Public (GUEST): no auth extractor needed.
//! pub async fn public_browse(...) -> impl IntoResponse { ... }
//! ```
//!
//! # Environment
//!
//! Set `JWT_SECRET` to a long random string in `.env`.
//! The application will panic on startup if `JWT_SECRET` is not set.

use std::sync::OnceLock;

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, EncodingKey, Validation};
use serde::{Deserialize, Serialize};

// ── JWT key cache ─────────────────────────────────────────────────────────────

static DECODING_KEY: OnceLock<DecodingKey> = OnceLock::new();
static ENCODING_KEY: OnceLock<EncodingKey> = OnceLock::new();

/// Parse and cache JWT keys from `JWT_SECRET`. Must be called once at startup
/// before the router is built. Panics if the env var is absent.
pub fn init_jwt_keys() {
    let secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET environment variable must be set");
    DECODING_KEY
        .set(DecodingKey::from_secret(secret.as_bytes()))
        .unwrap_or_else(|_| panic!("JWT keys already initialized"));
    ENCODING_KEY
        .set(EncodingKey::from_secret(secret.as_bytes()))
        .unwrap_or_else(|_| panic!("JWT keys already initialized"));
}

pub fn decoding_key() -> &'static DecodingKey {
    DECODING_KEY.get().expect("JWT keys not initialized")
}

pub fn encoding_key() -> &'static EncodingKey {
    ENCODING_KEY.get().expect("JWT keys not initialized")
}

// ── Role ─────────────────────────────────────────────────────────────────────

/// Application role. Maps 1-to-1 to the `role` column in the `users` table.
///
/// `Guest` access is represented by the *absence* of a JWT — there is no
/// corresponding DB value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Provider,
}

// ── JWT Claims ────────────────────────────────────────────────────────────────

/// Payload stored inside a signed JWT.
///
/// Generate tokens server-side (e.g. from a login handler) using
/// `jsonwebtoken::encode` with `EncodingKey::from_secret`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// `users.id` — the authenticated user's primary key.
    pub sub: i32,
    pub role: Role,
    pub username: String,
    /// Expiry as a Unix timestamp (seconds since epoch).
    /// `jsonwebtoken` validates this automatically when `validate_exp = true`.
    pub exp: usize,
}

// ── AuthUser ──────────────────────────────────────────────────────────────────

/// Represents a successfully authenticated caller.
///
/// Use this extractor in handlers that need to know *who* the caller is but
/// don't care about role. For role enforcement, prefer `RequireAdmin` or
/// `RequireProvider`.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id:   i32,
    pub role: Role,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Pull the raw token string from `Authorization: Bearer <token>`.
fn bearer_token(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}

/// Decode and validate a JWT, returning its claims on success.
fn decode_claims(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    decode::<Claims>(token, decoding_key(), &validation).map(|data| data.claims)
}

/// Build a JSON error response. Matches the `HandlerError` shape used
/// elsewhere in this codebase so clients get a consistent body format.
fn error_response(status: StatusCode, message: &'static str) -> Response {
    (status, Json(serde_json::json!({ "message": message }))).into_response()
}

// ── Extractor: AuthUser ───────────────────────────────────────────────────────

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    /// Returns a 401 JSON response when the token is absent or invalid.
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts)
            .ok_or_else(|| error_response(StatusCode::UNAUTHORIZED, "Authentication required."))?;

        let claims = decode_claims(token).map_err(|_| {
            error_response(
                StatusCode::UNAUTHORIZED,
                "Invalid or expired token.",
            )
        })?;

        Ok(AuthUser {
            id:   claims.sub,
            role: claims.role,
        })
    }
}

// ── Guard: RequireAdmin ───────────────────────────────────────────────────────

/// Extractor that succeeds only for `Role::Admin`.
///
/// Returns **401** if no valid token is present, **403** if the token
/// belongs to a Provider.
///
/// ```rust
/// pub async fn seed(RequireAdmin(user): RequireAdmin, ...) -> impl IntoResponse { ... }
/// ```
pub struct RequireAdmin(pub AuthUser);

impl<S: Send + Sync> FromRequestParts<S> for RequireAdmin {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;

        if user.role != Role::Admin {
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "Only administrators are allowed to perform this action.",
            ));
        }

        Ok(RequireAdmin(user))
    }
}

// ── Guard: RequireProvider ────────────────────────────────────────────────────

/// Extractor that succeeds for `Role::Admin` **or** `Role::Provider`.
///
/// Returns **401** if no valid token is present, **403** if the caller is
/// a plain Guest (no token).
///
/// ```rust
/// pub async fn update_listing(RequireProvider(user): RequireProvider, ...) -> impl IntoResponse {
///     // user.id lets you enforce "own resource" checks in the handler body:
///     // WHERE business.user_id = $user_id
/// }
/// ```
pub struct RequireProvider(pub AuthUser);

impl<S: Send + Sync> FromRequestParts<S> for RequireProvider {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;

        if !matches!(user.role, Role::Admin | Role::Provider) {
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "Access denied.",
            ));
        }

        Ok(RequireProvider(user))
    }
}
