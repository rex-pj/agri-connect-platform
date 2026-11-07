//! The JSON API of crop prices: the public board, and the admin's crops and
//! prices. The same use cases as the pages.

use application::authz::require_actor;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath, ApiQuery},
    middleware::auth::MaybeActor,
    requests::{
        AdminPageQuery, CropRequest, DeleteWithReasonBody, PriceBoardQuery, PriceRequest,
        ToggleActiveRequest,
    },
};

pub struct PriceHandler;

impl PriceHandler {
    /// GET /api/explore/prices — `location_id?`: each crop's latest fresh
    /// price per area, with the areas that have one.
    pub async fn board(
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<PriceBoardQuery>,
    ) -> Result<impl IntoResponse, Error> {
        Ok(Json(state.usecases.price.board(q.location_id).await?))
    }

    /// GET /api/admin/crops — every crop, hidden ones too.
    pub async fn list_crops(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.price.list_crops(&actor).await?))
    }

    /// POST /api/admin/crops — 201 with the crop.
    pub async fn create_crop(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<CropRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let prices = &state.usecases.price;
        let id = prices.create_crop(&actor, req.into()).await?;
        Ok((
            StatusCode::CREATED,
            Json(prices.get_crop(&actor, id).await?),
        ))
    }

    /// GET /api/admin/crops/{id}
    pub async fn get_crop(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.price.get_crop(&actor, id).await?))
    }

    /// PUT /api/admin/crops/{id}
    pub async fn update_crop(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<CropRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let prices = &state.usecases.price;
        prices.update_crop(&actor, id, req.into()).await?;
        Ok(Json(prices.get_crop(&actor, id).await?))
    }

    /// PATCH /api/admin/crops/{id}/status — `{is_active, reason?}`; 204.
    pub async fn set_crop_status(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ToggleActiveRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .price
            .set_crop_active(&actor, id, req.is_active, req.reason)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    /// GET /api/admin/prices — every price entered, newest day first (`page?`).
    pub async fn list_prices(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<AdminPageQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let page = q.page.unwrap_or(1);
        Ok(Json(state.usecases.price.list_prices(&actor, page).await?))
    }

    /// POST /api/admin/prices — `{crop_id, location_id, observed_on,
    /// price_low, price_high?, note?}`; the same crop, area and day replaces
    /// that day's price. 201 with `{id}`.
    pub async fn record_price(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<PriceRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let id = state
            .usecases
            .price
            .record_price(&actor, req.into())
            .await?;
        Ok((
            StatusCode::CREATED,
            Json(application::outputs::IdDto { id }),
        ))
    }

    /// DELETE /api/admin/prices/{id} — `{reason?}`; 204.
    pub async fn delete_price(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        body: Option<ApiJson<DeleteWithReasonBody>>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let reason = body.and_then(|b| b.0.reason);
        state
            .usecases
            .price
            .delete_price(&actor, id, reason)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }
}
