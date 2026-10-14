use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;
use serde::Serialize;

use crate::{app_state::AppState, middleware::auth::RequireAdmin};

pub struct SetupHandler;

#[derive(Serialize)]
struct DemoRemoved {
    removed_businesses: u64,
}

impl SetupHandler {
    /// GET /api/setup/status
    pub async fn get_status(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        Ok(Json(state.usecases.setup.get_status().await?))
    }

    /// POST /api/setup/seed — load demo listings; refused if already loaded.
    pub async fn seed(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        state.usecases.setup.load_demo().await?;
        Ok(Json(state.usecases.setup.get_status().await?))
    }

    /// DELETE /api/setup/seed — remove every demo listing, never a real one.
    pub async fn remove_seed(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let removed_businesses = state.usecases.setup.remove_demo().await?;
        Ok(Json(DemoRemoved { removed_businesses }))
    }
}
