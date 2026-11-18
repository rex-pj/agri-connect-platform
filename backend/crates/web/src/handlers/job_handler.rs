//! The JSON API of job posts: a farmer's post, a provider's jobs nearby,
//! and the admin's list and states. The same use cases as the pages.

use application::{authz::require_actor, outputs::IdDto};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath, ApiQuery},
    middleware::auth::MaybeActor,
    requests::{JobRequest, JobStatusRequest, StatusListQuery},
};

pub struct JobHandler;

impl JobHandler {
    /// POST /api/explore/jobs — `{category_id, location_id?, lat?, lng?,
    /// description, requester_name, requester_phone}`; 201 `{id}`.
    pub async fn post(
        ip: Option<axum::Extension<crate::middleware::client_ip::ClientIp>>,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<JobRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let client = crate::middleware::client_ip::client_key(ip.as_deref());
        let id = state.usecases.job.post(req.into_input(client)).await?;
        Ok((StatusCode::CREATED, Json(IdDto { id })))
    }

    /// GET /api/provider/jobs — open posts near the own business.
    pub async fn list_own(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.job.list_own(&actor).await?))
    }

    /// GET /api/admin/jobs — every post, newest first (`page?`, `status?`).
    pub async fn list_admin(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<StatusListQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let page = q.page.unwrap_or(1);
        Ok(Json(
            state
                .usecases
                .job
                .list_admin(&actor, page, q.status.as_deref())
                .await?,
        ))
    }

    /// PATCH /api/admin/jobs/{id}/status — `{status, reason?}`; 204.
    pub async fn set_status(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<JobStatusRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .job
            .set_status(&actor, id, &req.status, req.reason)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }
}
