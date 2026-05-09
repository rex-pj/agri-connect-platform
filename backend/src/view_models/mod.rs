pub mod requests;
pub mod responses;

use std::collections::HashMap;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

// ── Application error (returned by use cases) ─────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    /// Account is temporarily locked after too many failed login attempts.
    /// Maps to HTTP 423 so the client can distinguish it from a wrong password.
    #[error("{0}")]
    AccountLocked(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Internal(String),
}

impl From<AppError> for HandlerError {
    fn from(e: AppError) -> Self {
        match e {
            AppError::NotFound(msg)      => HandlerError::not_found(msg),
            AppError::BadRequest(msg)    => HandlerError::bad_request(msg),
            AppError::Unauthorized(msg)  => HandlerError::unauthorized(msg),
            AppError::AccountLocked(msg) => HandlerError::account_locked(msg),
            AppError::Forbidden(msg)     => HandlerError::forbidden(msg),
            AppError::Conflict(msg)      => HandlerError::conflict(msg),
            AppError::Internal(msg)      => HandlerError::internal(msg),
        }
    }
}

// ── Error payload sent to the client ──────────────────────────────────────────

#[derive(Serialize)]
pub struct HandlerErrorResponse {
    pub error: String,
    pub field_errors: Option<HashMap<String, String>>,
}

// ── Internal error carrier (mirrors rex_game HandlerError) ────────────────────

#[derive(Default)]
pub struct HandlerError {
    pub status: StatusCode,
    pub message: String,
    pub field_errors: Option<HashMap<String, String>>,
}

impl HandlerError {
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
            ..Default::default()
        }
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
            ..Default::default()
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
            ..Default::default()
        }
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: msg.into(),
            ..Default::default()
        }
    }

    pub fn account_locked(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::LOCKED,   // 423
            message: msg.into(),
            ..Default::default()
        }
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: msg.into(),
            ..Default::default()
        }
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: msg.into(),
            ..Default::default()
        }
    }

    pub fn validation(msg: impl Into<String>, errors: validator::ValidationErrors) -> Self {
        use crate::validators::ValidationHelper;
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
            field_errors: Some(ValidationHelper::flatten_errors(errors)),
        }
    }
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> Response {
        let body = Json(HandlerErrorResponse {
            error: self.message,
            field_errors: self.field_errors,
        });
        (self.status, body).into_response()
    }
}

pub type HandlerResult<T> = Result<T, HandlerError>;
