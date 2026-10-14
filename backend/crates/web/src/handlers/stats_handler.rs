use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;

use crate::{app_state::AppState, middleware::auth::RequireAdmin};

pub struct StatsHandler;

impl StatsHandler {
    /// GET /api/admin/stats
    pub async fn get_stats(
        RequireAdmin(_caller): RequireAdmin,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state
            .usecases
            .stats
            .get_stats()
            .await?;

        Ok(Json(result))
    }
}
