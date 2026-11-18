//! The JSON API of the platform settings (`setting.manage`): the same use
//! case as `/admin/settings`.

use std::collections::BTreeMap;

use application::authz::require_actor;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;
use serde::Deserialize;

use crate::{app_state::AppState, extract::ApiJson, middleware::auth::MaybeActor};

/// PATCH /api/admin/settings: the values to change, by key, as text
/// (`"20"`, `"true"`, `"0912345678"`, `""` to clear a text one).
#[derive(Debug, Deserialize)]
pub struct SettingsRequest {
    pub values: BTreeMap<String, String>,
    #[serde(default)]
    pub reason: Option<String>,
}

pub struct SettingsHandler;

impl SettingsHandler {
    /// GET /api/admin/settings: every setting by group, with its default,
    /// bounds and when it was changed.
    pub async fn list(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.settings.groups(&actor).await?))
    }

    /// PATCH /api/admin/settings: 204; every broken value is a field error
    /// on its key.
    pub async fn update(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<SettingsRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let values: Vec<(String, String)> = req.values.into_iter().collect();
        state
            .usecases
            .settings
            .update(&actor, &values, req.reason)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }
}
