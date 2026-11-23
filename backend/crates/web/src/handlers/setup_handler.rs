use application::authz::require_actor;
use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;
use serde::Serialize;

use crate::{app_state::AppState, middleware::auth::MaybeActor};

pub struct SetupHandler;

#[derive(Serialize)]
struct DemoRemoved {
    removed_businesses: u64,
}

impl SetupHandler {
    /// GET /api/setup/status
    pub async fn get_status(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.setup.get_status(&actor).await?))
    }

    /// POST /api/setup/seed — load demo listings; refused if already loaded.
    pub async fn seed(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state.usecases.setup.load_demo(&actor).await?;
        Ok(Json(state.usecases.setup.get_status(&actor).await?))
    }

    /// DELETE /api/setup/seed — remove every demo listing, never a real one.
    pub async fn remove_seed(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let removed_businesses = state.usecases.setup.remove_demo(&actor).await?;
        Ok(Json(DemoRemoved { removed_businesses }))
    }
}
