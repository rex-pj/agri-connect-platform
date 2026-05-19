use axum::{extract::{Path, State}, http::StatusCode, Json};

use crate::{
    app_state::AppState,
    middleware::auth::AuthUser,
    view_models::{
        requests::{
            ChangePasswordRequest, CreateAdminRequest, ForgotPasswordRequest, LoginRequest,
            RegisterRequest, ResetPasswordRequest,
        },
        HandlerError, HandlerResult,
    },
};

pub struct AuthHandler;

impl AuthHandler {
    /// POST /api/auth/login
    pub async fn login(
        State(state): State<AppState>,
        Json(req): Json<LoginRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .auth
            .login(req.username, req.password)
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// POST /api/admin/initialize  — create the very first admin account
    pub async fn create_first_admin(
        State(state): State<AppState>,
        Json(req): Json<CreateAdminRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .auth
            .create_first_admin(req.username, req.email, req.password, req.full_name)
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// POST /api/auth/register  — provider self-registration
    pub async fn register(
        State(state): State<AppState>,
        Json(req): Json<RegisterRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .auth
            .register(
                req.username,
                req.email,
                req.password,
                req.full_name,
                req.business_name,
                req.business_phone,
            )
            .await
            .map_err(HandlerError::from)?;

        Ok((StatusCode::CREATED, Json(result)))
    }

    /// GET /api/users/me  — return the authenticated user's own profile
    pub async fn me(
        auth_user: AuthUser,
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .auth
            .me(auth_user.id)
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// PATCH /api/users/me/password  — change password (authenticated)
    pub async fn change_password(
        auth_user: AuthUser,
        State(state): State<AppState>,
        Json(req): Json<ChangePasswordRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state
            .usecases
            .auth
            .change_password(auth_user.id, req.current_password, req.new_password)
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(serde_json::json!({ "message": "Password changed successfully." })))
    }

    /// POST /api/auth/password-resets  — request a reset email
    pub async fn forgot_password(
        State(state): State<AppState>,
        Json(req): Json<ForgotPasswordRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state
            .usecases
            .auth
            .forgot_password(req.email)
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(serde_json::json!({
            "message": "If the email exists in our system, we have sent password reset instructions."
        })))
    }

    /// PATCH /api/auth/password-resets/{token}  — consume token and set new password
    pub async fn reset_password(
        State(state): State<AppState>,
        Path(token): Path<String>,
        Json(req): Json<ResetPasswordRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state
            .usecases
            .auth
            .reset_password(token, req.new_password)
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(serde_json::json!({ "message": "Password reset successfully. Please log in again." })))
    }
}
