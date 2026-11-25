use application::{authz::require_actor, inputs::RegisterInput};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath},
    middleware::auth::{CallerSession, MaybeActor},
    requests::{
        ChangeEmailRequest, ChangePasswordRequest, CreateAdminRequest, ForgotPasswordRequest,
        LoginRequest, RegisterRequest, ResetPasswordRequest, UpdateMeRequest,
    },
};

pub struct AuthHandler;

impl AuthHandler {
    /// POST /api/auth/tokens
    pub async fn login(
        ip: Option<axum::Extension<crate::middleware::client_ip::ClientIp>>,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<LoginRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let client = crate::middleware::client_ip::client_key(ip.as_deref());
        let result = state
            .usecases
            .auth
            .login(req.email, req.password, &client)
            .await?;

        Ok(Json(result))
    }

    /// DELETE /api/auth/tokens — log out: the caller's token (and its
    /// session) stops working at once. 204, also for a session already ended.
    pub async fn logout(
        CallerSession(claims): CallerSession,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        state.usecases.auth.end_token_session(&claims).await?;
        Ok(StatusCode::NO_CONTENT)
    }

    /// POST /api/admin/setup  — create the very first admin account
    pub async fn create_first_admin(
        State(state): State<AppState>,
        ApiJson(req): ApiJson<CreateAdminRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state
            .usecases
            .auth
            .create_first_admin(req.email, req.password, req.full_name, req.setup_token)
            .await?;

        Ok(Json(result))
    }

    /// POST /api/users  — provider self-registration
    pub async fn register(
        ip: Option<axum::Extension<crate::middleware::client_ip::ClientIp>>,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<RegisterRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state
            .usecases
            .auth
            .register(RegisterInput {
                email: req.email,
                password: req.password,
                full_name: req.full_name,
                business_name: req.business_name,
                business_phone: req.business_phone,
                client: crate::middleware::client_ip::client_key(ip.as_deref()),
            })
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

    /// PATCH /api/users/me — the name the account is shown by (`full_name`,
    /// blank or absent clears it); 204.
    pub async fn update_me(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<UpdateMeRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .auth
            .update_profile(actor.user_id, req.full_name)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    /// POST /api/users/me/email — move to `new_email` (`current_password`);
    /// 202: nothing changes until the link sent there is opened.
    pub async fn change_email(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<ChangeEmailRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .auth
            .request_email_change(actor.user_id, req.current_password, req.new_email)
            .await?;
        Ok(StatusCode::ACCEPTED)
    }

    /// DELETE /api/users/me/sessions — every session of the account ends,
    /// the caller's too; 204.
    pub async fn end_sessions(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .auth
            .end_other_sessions(actor.user_id)
            .await?;
        Ok(StatusCode::NO_CONTENT)
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
