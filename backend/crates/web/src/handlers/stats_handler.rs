use application::authz::require_actor;
use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;

use crate::{app_state::AppState, middleware::auth::MaybeActor};

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
}
