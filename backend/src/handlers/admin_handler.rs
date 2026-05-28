use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};


use crate::{
    app_state::AppState,
    middleware::auth::RequireAdmin,
    view_models::{
        requests::{
            AdminCreateServiceRequest, AdminListQuery, AdminPageQuery, AssignUserRequest,
            CreateCategoryRequest, CreateLocationRequest, DeleteWithReasonBody,
            ToggleActiveRequest, ToggleVerifyRequest, ToggleVisibleRequest,
            UpdateCategoryRequest, UpdateInquiryStatusRequest, UpdateLocationRequest,
            UpdateUserRoleRequest, AdminUpdateServiceRequest,
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
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        body: Option<Json<ToggleVerifyRequest>>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let reason = body.and_then(|b| b.0.reason);
        let result = state.usecases.admin.toggle_verify(id, admin.id, reason).await.map_err(HandlerError::from)?;
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
        Ok((StatusCode::CREATED, Json(result)))
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
        let result = state.usecases.admin.update_user_role(id, req.role_code).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/users/{id}/status
    pub async fn toggle_user_active(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<ToggleActiveRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.toggle_user_active(id, req.is_active, admin.id, req.reason).await.map_err(HandlerError::from)?;
        Ok(Json(result))
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
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Json(req): Json<CreateCategoryRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .admin
            .create_category(req.name, req.slug, req.category_type, req.icon, req.display_order, admin.id)
            .await
            .map_err(HandlerError::from)?;
        Ok((StatusCode::CREATED, Json(result)))
    }

    /// PUT /api/admin/categories/{id}
    pub async fn update_category(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<UpdateCategoryRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .admin
            .update_category(id, req.name, req.slug, req.icon, req.display_order, admin.id)
            .await
            .map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/categories/{id}/status
    pub async fn toggle_category_active(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<ToggleActiveRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.toggle_category_active(id, req.is_active, admin.id, req.reason).await.map_err(HandlerError::from)?;
        Ok(Json(result))
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
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<UpdateInquiryStatusRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.update_inquiry_status(id, req.status, admin.id).await.map_err(HandlerError::from)?;
        Ok(Json(result))
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

    /// PATCH /api/admin/reviews/{id}/visibility
    pub async fn toggle_review_visible(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<ToggleVisibleRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.toggle_review_visible(id, req.is_visible, admin.id, req.reason).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    // ── Locations ─────────────────────────────────────────────────────────────

    /// GET /api/admin/locations
    pub async fn list_locations(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.list_locations_admin().await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// POST /api/admin/locations
    pub async fn create_location(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Json(req): Json<CreateLocationRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin
            .create_location(req.label, req.lat, req.lng, req.display_order)
            .await
            .map_err(HandlerError::from)?;
        Ok((StatusCode::CREATED, Json(result)))
    }

    /// GET /api/admin/locations/{id}
    pub async fn get_location(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.get_location_admin(id).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// PUT /api/admin/locations/{id}
    pub async fn update_location(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<UpdateLocationRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin
            .update_location(id, req.label, req.lat, req.lng, req.display_order)
            .await
            .map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/locations/{id}/status
    pub async fn toggle_location_active(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<ToggleActiveRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.toggle_location_active(id, req.is_active, admin.id, req.reason).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// DELETE /api/admin/locations/{id}
    pub async fn delete_location(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        body: Option<Json<DeleteWithReasonBody>>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let reason = body.and_then(|b| b.0.reason);
        state.usecases.admin.delete_location(id, admin.id, reason).await.map_err(HandlerError::from)?;
        Ok(StatusCode::NO_CONTENT)
    }

    // ── Admin Products ────────────────────────────────────────────────────────

    /// GET /api/admin/products
    pub async fn list_products(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Query(q): Query<AdminListQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin
            .list_products_admin(q.page.unwrap_or(1), q.provider_id)
            .await
            .map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// GET /api/admin/products/{id}
    pub async fn get_product(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.get_product_admin(id).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/products/{id}/active
    pub async fn toggle_product_active(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<ToggleActiveRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin
            .toggle_product_active(id, req.is_active, admin.id, req.reason)
            .await
            .map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// DELETE /api/admin/products/{id}
    pub async fn delete_product(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        body: Option<Json<DeleteWithReasonBody>>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let reason = body.and_then(|b| b.0.reason);
        state.usecases.admin.delete_product_admin(id, admin.id, reason).await.map_err(HandlerError::from)?;
        Ok(StatusCode::NO_CONTENT)
    }

    // ── Admin Services ────────────────────────────────────────────────────────

    /// GET /api/admin/services
    pub async fn list_services(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Query(q): Query<AdminListQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin
            .list_services_admin(q.page.unwrap_or(1), q.provider_id)
            .await
            .map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// GET /api/admin/services/{id}
    pub async fn get_service(
        RequireAdmin(_admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin.get_service_admin(id).await.map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// POST /api/admin/services
    pub async fn create_service(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Json(req): Json<AdminCreateServiceRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin
            .create_service_admin(
                req.business_id, req.category_id, req.name,
                req.description, req.price_note, admin.id,
            )
            .await
            .map_err(HandlerError::from)?;
        Ok((StatusCode::CREATED, Json(result)))
    }

    /// PUT /api/admin/services/{id}
    pub async fn update_service(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<AdminUpdateServiceRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin
            .update_service_admin(
                id, req.category_id, req.name,
                req.description, req.price_note, admin.id,
            )
            .await
            .map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/services/{id}/active
    pub async fn toggle_service_active(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<ToggleActiveRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state.usecases.admin
            .toggle_service_active(id, req.is_active, admin.id, req.reason)
            .await
            .map_err(HandlerError::from)?;
        Ok(Json(result))
    }

    /// DELETE /api/admin/services/{id}
    pub async fn delete_service(
        RequireAdmin(admin): RequireAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        body: Option<Json<DeleteWithReasonBody>>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let reason = body.and_then(|b| b.0.reason);
        state.usecases.admin.delete_service_admin(id, admin.id, reason).await.map_err(HandlerError::from)?;
        Ok(StatusCode::NO_CONTENT)
    }
}
