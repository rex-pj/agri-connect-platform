use axum::{extract::State, Json};

use crate::{
    app_state::AppState,
    middleware::auth::RequireAdmin,
    view_models::{HandlerError, HandlerResult},
};

pub struct SetupHandler;

impl SetupHandler {
    /// GET /api/setup/status
    pub async fn get_status(
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .setup
            .get_status()
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// POST /api/setup/data — requires admin JWT
    pub async fn seed(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .setup
            .verify_seed()
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }
}
