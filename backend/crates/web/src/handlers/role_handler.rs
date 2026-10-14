use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;

use crate::{app_state::AppState, extract::ApiPath, middleware::auth::RequireAdmin};

pub struct RoleHandler;

impl RoleHandler {
    /// GET /api/admin/roles
    pub async fn list(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state.usecases.admin.list_roles().await?;
        Ok(Json(result))
    }

    /// GET /api/admin/roles/{id}
    pub async fn get_by_id(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state.usecases.admin.get_role_by_id(id).await?;
        Ok(Json(result))
    }
}
