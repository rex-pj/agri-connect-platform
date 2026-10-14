use application::authz::require_actor;
use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;

use crate::{app_state::AppState, extract::ApiPath, middleware::auth::MaybeActor};

pub struct PermissionHandler;

impl PermissionHandler {
    /// GET /api/admin/permissions
    pub async fn list_all(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.permission.list_all(&actor).await?;
        Ok(Json(result))
    }

    /// GET /api/admin/users/{id}/permissions
    pub async fn list_by_user(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(user_id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .permission
            .list_by_user(&actor, user_id)
            .await?;
        Ok(Json(result))
    }
}
