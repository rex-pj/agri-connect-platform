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
    pub message:      String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_errors: Option<HashMap<String, Vec<String>>>,
}

// ── Internal error carrier ────────────────────────────────────────────────────

#[derive(Default)]
pub struct HandlerError {
    pub status:       StatusCode,
    pub message:      String,
    pub field_errors: Option<HashMap<String, Vec<String>>>,
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

    /// Build a 400 response from `validator::ValidationErrors`.
    ///
    /// Error formatting is inlined here so `view_models` does not import from
    /// the `validators` utility module (S3 — presentational layer must not
    /// depend on cross-cutting validators).
    pub fn validation(msg: impl Into<String>, errors: validator::ValidationErrors) -> Self {
        let field_errors = errors
            .field_errors()
            .into_iter()
            .map(|(field, errs)| {
                let msgs: Vec<String> = errs
                    .iter()
                    .map(|e| {
                        e.message
                            .as_ref()
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| format!("Invalid value: {field}"))
                    })
                    .collect();
                (field.to_string(), msgs)
            })
            .collect();
        Self {
            status:       StatusCode::BAD_REQUEST,
            message:      msg.into(),
            field_errors: Some(field_errors),
        }
    }
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> Response {
        let body = Json(HandlerErrorResponse {
            message:     self.message,
            field_errors: self.field_errors,
        });
        (self.status, body).into_response()
    }
}

pub type HandlerResult<T> = Result<T, HandlerError>;
