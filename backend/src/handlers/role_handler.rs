use axum::extract::{Path, State};
use axum::Json;

use crate::{
    app_state::AppState,
    middleware::auth::RequireAdmin,
    view_models::{HandlerError, HandlerResult},
};

pub struct RoleHandler;

impl RoleHandler {
    /// GET /api/admin/roles
    pub async fn list(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.role.list().await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// GET /api/admin/roles/{id}
    pub async fn get_by_id(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.role.get_by_id(id).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }
}
