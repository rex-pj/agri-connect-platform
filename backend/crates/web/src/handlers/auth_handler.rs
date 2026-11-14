use application::authz::require_actor;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath},
    middleware::auth::MaybeActor,
    requests::{
        ChangePasswordRequest, CreateAdminRequest, ForgotPasswordRequest, LoginRequest,
        RegisterRequest, ResetPasswordRequest,
    },
};

pub struct AuthHandler;

impl AuthHandler {
    /// POST /api/auth/login
    pub async fn login(
        State(state): State<AppState>,
        ApiJson(req): ApiJson<LoginRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state
            .usecases
            .auth
            .login(req.username, req.password)
            .await?;

        Ok(Json(result))
    }

    /// POST /api/admin/initialize  — create the very first admin account
    pub async fn create_first_admin(
        State(state): State<AppState>,
        ApiJson(req): ApiJson<CreateAdminRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state
            .usecases
            .auth
            .create_first_admin(
                req.username,
                req.email,
                req.password,
                req.full_name,
                req.setup_token,
            )
            .await?;

        Ok(Json(result))
    }

    /// POST /api/auth/register  — provider self-registration
    pub async fn register(
        State(state): State<AppState>,
        ApiJson(req): ApiJson<RegisterRequest>,
    ) -> Result<impl IntoResponse, Error> {
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
            .await?;

        Ok((StatusCode::CREATED, Json(result)))
    }

    /// GET /api/users/me  — return the authenticated user's own profile
    pub async fn me(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.auth.me(actor.user_id).await?;

        Ok(Json(result))
    }

    /// PATCH /api/users/me/password  — change password (authenticated)
    pub async fn change_password(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<ChangePasswordRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .auth
            .change_password(actor.user_id, req.current_password, req.new_password)
            .await?;

        Ok(StatusCode::NO_CONTENT)
    }

    /// POST /api/auth/password-resets  — request a reset email
    pub async fn forgot_password(
        State(state): State<AppState>,
        ApiJson(req): ApiJson<ForgotPasswordRequest>,
    ) -> Result<impl IntoResponse, Error> {
        state.usecases.auth.forgot_password(req.email).await?;

        // Same answer whether or not the address exists.
        Ok(StatusCode::NO_CONTENT)
    }

    /// PATCH /api/auth/password-resets/{token}  — consume token and set new password
    pub async fn reset_password(
        State(state): State<AppState>,
        ApiPath(token): ApiPath<String>,
        ApiJson(req): ApiJson<ResetPasswordRequest>,
    ) -> Result<impl IntoResponse, Error> {
        state
            .usecases
            .auth
            .reset_password(token, req.new_password)
            .await?;

        Ok(StatusCode::NO_CONTENT)
    }
}
