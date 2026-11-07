use application::authz::require_actor;
use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState, extract::ApiQuery, middleware::auth::MaybeActor, requests::DemandQuery,
};

pub struct StatsHandler;

impl StatsHandler {
    /// GET /api/admin/stats
    pub async fn get_stats(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.stats.get_stats(&actor).await?;

        Ok(Json(result))
    }

    /// GET /api/admin/reports/demand — `area?` a location id or `none`; the
    /// same figures as the page, small ones already hidden (`value: null`).
    pub async fn demand_report(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<DemandQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.demand.report(&actor, q.focus()).await?;
        Ok(Json(result))
    }
}
