use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;

use crate::{app_state::AppState, extract::ApiPath, middleware::auth::RequireAdmin};

pub struct PermissionHandler;

impl PermissionHandler {
    /// GET /api/admin/permissions
    pub async fn list_all(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state.usecases.permission.list_all().await?;
        Ok(Json(result))
    }

    /// GET /api/admin/users/{id}/permissions
    pub async fn list_by_user(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        ApiPath(user_id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state.usecases.permission.list_by_user(user_id).await?;
        Ok(Json(result))
    }
}
