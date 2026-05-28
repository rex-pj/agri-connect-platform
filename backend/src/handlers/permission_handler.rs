use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    app_state::AppState,
    middleware::auth::RequireAdmin,
    view_models::{requests::AssignPermissionRequest, HandlerError, HandlerResult},
};

pub struct PermissionHandler;

impl PermissionHandler {
    /// GET /api/admin/permissions
    pub async fn list_all(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.permission.list_all().await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// GET /api/admin/users/{id}/permissions
    pub async fn list_by_user(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(user_id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.permission.list_by_user(user_id).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// POST /api/admin/users/{id}/permissions
    pub async fn assign(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(user_id): Path<i32>,
        Json(req): Json<AssignPermissionRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state.usecases.permission
            .assign(user_id, req.permission_id, admin.id)
            .await
            .map_err(HandlerError::from)?;
        Ok(StatusCode::NO_CONTENT)
    }

    /// DELETE /api/admin/users/{id}/permissions/{pid}
    pub async fn revoke(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path((user_id, permission_id)): Path<(i32, i32)>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state.usecases.permission
            .revoke(user_id, permission_id)
            .await
            .map_err(HandlerError::from)?;
        Ok(StatusCode::NO_CONTENT)
    }
}
