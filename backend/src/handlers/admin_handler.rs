use axum::{
    extract::{Path, Query, State},
    Json,
};

use crate::{
    app_state::AppState,
    middleware::auth::RequireAdmin,
    view_models::{
        requests::{
            AdminPageQuery, AssignUserRequest, CreateCategoryRequest, ToggleActiveRequest,
            ToggleVisibleRequest, UpdateCategoryRequest, UpdateInquiryStatusRequest,
            UpdateUserRoleRequest,
        },
        HandlerError, HandlerResult,
    },
};

pub struct AdminHandler;

impl AdminHandler {
    /// GET /api/admin/providers/{id}
    pub async fn get_provider_detail(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.get_provider_detail(id).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// GET /api/admin/providers
    pub async fn list_providers(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Query(q): Query<AdminPageQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.list_providers(q.page.unwrap_or(1)).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/providers/{id}/verification
    pub async fn toggle_verify(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.toggle_verify(id).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// POST /api/admin/providers/{id}/users
    pub async fn assign_provider_user(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<AssignUserRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .admin
            .assign_provider_user(id, req.username, req.email, req.password)
            .await
            .map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// GET /api/admin/users/{id}
    pub async fn get_user_detail(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.get_user_detail(id).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// GET /api/admin/users
    pub async fn list_users(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Query(q): Query<AdminPageQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.list_users(q.page.unwrap_or(1)).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/users/{id}/role
    pub async fn update_user_role(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<UpdateUserRoleRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state.usecases.admin.update_user_role(id, req.role_code).await.map_err(HandlerError::from)?;
        Ok(Json(serde_json::json!({ "success": true })))
    }

    /// PATCH /api/admin/users/{id}/active
    pub async fn toggle_user_active(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<ToggleActiveRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state.usecases.admin.toggle_user_active(id, req.is_active).await.map_err(HandlerError::from)?;
        Ok(Json(serde_json::json!({ "success": true })))
    }

    /// GET /api/admin/categories/{id}
    pub async fn get_category_detail(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.get_category_detail(id).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// GET /api/admin/categories
    pub async fn list_categories(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.list_categories().await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// POST /api/admin/categories
    pub async fn create_category(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Json(req): Json<CreateCategoryRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let id = state
            .usecases
            .admin
            .create_category(req.name, req.slug, req.category_type, req.icon, req.display_order)
            .await
            .map_err(HandlerError::from)?;
        Ok(Json(serde_json::json!({ "id": id })))
    }

    /// PUT /api/admin/categories/{id}
    pub async fn update_category(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<UpdateCategoryRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state
            .usecases
            .admin
            .update_category(id, req.name, req.slug, req.icon, req.display_order)
            .await
            .map_err(HandlerError::from)?;
        Ok(Json(serde_json::json!({ "success": true })))
    }

    /// PATCH /api/admin/categories/{id}/active
    pub async fn toggle_category_active(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<ToggleActiveRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state.usecases.admin.toggle_category_active(id, req.is_active).await.map_err(HandlerError::from)?;
        Ok(Json(serde_json::json!({ "success": true })))
    }

    /// GET /api/admin/inquiries
    pub async fn list_inquiries(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Query(q): Query<AdminPageQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.list_inquiries(q.page.unwrap_or(1)).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/inquiries/{id}/status
    pub async fn update_inquiry_status(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<UpdateInquiryStatusRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state.usecases.admin.update_inquiry_status(id, req.status).await.map_err(HandlerError::from)?;
        Ok(Json(serde_json::json!({ "success": true })))
    }

    /// GET /api/admin/reviews
    pub async fn list_reviews(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Query(q): Query<AdminPageQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.list_reviews(q.page.unwrap_or(1)).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/reviews/{id}/visible
    pub async fn toggle_review_visible(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<ToggleVisibleRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state.usecases.admin.toggle_review_visible(id, req.is_visible).await.map_err(HandlerError::from)?;
        Ok(Json(serde_json::json!({ "success": true })))
    }
}
