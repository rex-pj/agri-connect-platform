//! The JSON API of commodities (cây trồng, vật nuôi): the public list the clients
//! filter supplies and read the price board by, and the admin's list. The
//! same use case as the pages.

use application::authz::require_actor;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath},
    middleware::auth::MaybeActor,
    requests::{CommodityRequest, ToggleActiveRequest},
};

pub struct CommodityHandler;

impl CommodityHandler {
    /// GET /api/explore/commodities — the shown ones, by sector and order.
    pub async fn shown(State(state): State<AppState>) -> Result<impl IntoResponse, Error> {
        Ok(Json(state.usecases.commodity.shown().await?))
    }

    /// GET /api/admin/commodities — every one, hidden ones too.
    pub async fn list(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.commodity.list(&actor).await?))
    }

    /// POST /api/admin/commodities — 201 with the commodity.
    pub async fn create(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<CommodityRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let commodities = &state.usecases.commodity;
        let id = commodities.create(&actor, req.into()).await?;
        Ok((
            StatusCode::CREATED,
            Json(commodities.get(&actor, id).await?),
        ))
    }

    /// GET /api/admin/commodities/{id}
    pub async fn get(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.commodity.get(&actor, id).await?))
    }

    /// PUT /api/admin/commodities/{id}
    pub async fn update(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<CommodityRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let commodities = &state.usecases.commodity;
        commodities.update(&actor, id, req.into()).await?;
        Ok(Json(commodities.get(&actor, id).await?))
    }

    /// PATCH /api/admin/commodities/{id}/status — `{is_active, reason?}`; 204.
    pub async fn set_status(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ToggleActiveRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .commodity
            .set_active(&actor, id, req.is_active, req.reason)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }
}
