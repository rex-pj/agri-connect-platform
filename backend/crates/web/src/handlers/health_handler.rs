use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;
use serde::Serialize;

use crate::app_state::AppState;

pub struct HealthHandler;

#[derive(Serialize)]
struct Status {
    status: &'static str,
}

impl HealthHandler {
    /// GET /health/live — the process answers. Checks nothing else, so a
    /// database outage never makes the orchestrator restart a healthy process.
    pub async fn live() -> impl IntoResponse {
        Json(Status { status: "live" })
    }

    /// GET /health/ready — 503 only when the database is unreachable.
    pub async fn ready(State(state): State<AppState>) -> Result<impl IntoResponse, Error> {
        state.usecases.health.ready().await?;
        Ok(Json(Status { status: "ready" }))
    }
}
