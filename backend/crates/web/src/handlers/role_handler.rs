use application::authz::require_actor;
use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;

use crate::{app_state::AppState, extract::ApiPath, middleware::auth::MaybeActor};

pub struct RoleHandler;

impl RoleHandler {
    /// GET /api/admin/roles
    pub async fn list(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.admin.list_roles(&actor).await?;
        Ok(Json(result))
    }

    /// GET /api/admin/roles/{id}
    pub async fn get_by_id(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.admin.get_role_by_id(&actor, id).await?;
        Ok(Json(result))
    }
}
