use axum::{extract::State, Json};

use crate::{
    app_state::AppState,
    middleware::auth::RequireAdmin,
    view_models::{HandlerError, HandlerResult},
};

pub struct StatsHandler;

impl StatsHandler {
    /// GET /api/admin/stats
    pub async fn get_stats(
        RequireAdmin(_caller): RequireAdmin,
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .stats
            .get_stats()
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }
}
