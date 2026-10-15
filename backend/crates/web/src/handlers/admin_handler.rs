use application::authz::require_actor;
use application::inputs::CategoryInput;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath, ApiQuery},
    middleware::auth::MaybeActor,
    requests::{
        AdminCreateServiceRequest, AdminListQuery, AdminPageQuery, AdminUpdateServiceRequest,
        AssignUserRequest, CreateCategoryRequest, CreateLocationRequest, DeleteWithReasonBody,
        SetVerificationRequest, ToggleActiveRequest, ToggleVisibleRequest, UpdateCategoryRequest,
        UpdateInquiryStatusRequest, UpdateLocationRequest, UpdateUserRoleRequest,
    },
};

pub struct AdminHandler;

impl AdminHandler {
    /// GET /api/admin/providers/{id}
    pub async fn get_provider_detail(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.admin.get_provider_detail(&actor, id).await?;
        Ok(Json(result))
    }

    /// GET /api/admin/providers
    pub async fn list_providers(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<AdminPageQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .list_providers(&actor, q.page.unwrap_or(1))
            .await?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/providers/{id}/verification
    pub async fn set_verification(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<SetVerificationRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .set_verification(&actor, id, req.is_verified, req.reason)
            .await?;
        Ok(Json(result))
    }

    /// POST /api/admin/providers/{id}/users
    pub async fn assign_provider_user(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<AssignUserRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .assign_provider_user(&actor, id, req.username, req.email, req.password)
            .await?;
        Ok((StatusCode::CREATED, Json(result)))
    }

    /// GET /api/admin/users/{id}
    pub async fn get_user_detail(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.admin.get_user_detail(&actor, id).await?;
        Ok(Json(result))
    }

    /// GET /api/admin/users
    pub async fn list_users(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<AdminPageQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .list_users(&actor, q.page.unwrap_or(1))
            .await?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/users/{id}/role
    pub async fn update_user_role(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<UpdateUserRoleRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .update_user_role(&actor, id, req.role_code)
            .await?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/users/{id}/status
    pub async fn toggle_user_active(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ToggleActiveRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .toggle_user_active(&actor, id, req.is_active, req.reason)
            .await?;
        Ok(Json(result))
    }

    /// GET /api/admin/categories/{id}
    pub async fn get_category_detail(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.admin.get_category_detail(&actor, id).await?;
        Ok(Json(result))
    }

    /// GET /api/admin/categories
    pub async fn list_categories(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.admin.list_categories(&actor).await?;
        Ok(Json(result))
    }

    /// POST /api/admin/categories
    pub async fn create_category(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<CreateCategoryRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .create_category(
                &actor,
                req.category_type,
                CategoryInput {
                    name: req.name,
                    slug: req.slug,
                    icon: req.icon,
                    color: req.color,
                    description: req.description,
                    display_order: req.display_order,
                },
            )
            .await?;
        Ok((StatusCode::CREATED, Json(result)))
    }

    /// PUT /api/admin/categories/{id}
    pub async fn update_category(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<UpdateCategoryRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .update_category(
                &actor,
                id,
                CategoryInput {
                    name: req.name,
                    slug: req.slug,
                    icon: req.icon,
                    color: req.color,
                    description: req.description,
                    display_order: req.display_order,
                },
            )
            .await?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/categories/{id}/status
    pub async fn toggle_category_active(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ToggleActiveRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .toggle_category_active(&actor, id, req.is_active, req.reason)
            .await?;
        Ok(Json(result))
    }

    /// GET /api/admin/inquiries
    pub async fn list_inquiries(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<AdminPageQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .list_inquiries(&actor, q.page.unwrap_or(1))
            .await?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/inquiries/{id}/status
    pub async fn update_inquiry_status(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<UpdateInquiryStatusRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .update_inquiry_status(&actor, id, req.status)
            .await?;
        Ok(Json(result))
    }

    /// GET /api/admin/reviews
    pub async fn list_reviews(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<AdminPageQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .list_reviews(&actor, q.page.unwrap_or(1))
            .await?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/reviews/{id}/visibility
    pub async fn toggle_review_visible(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ToggleVisibleRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .toggle_review_visible(&actor, id, req.is_visible, req.reason)
            .await?;
        Ok(Json(result))
    }

    // ── Locations ─────────────────────────────────────────────────────────────

    /// GET /api/admin/locations
    pub async fn list_locations(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.admin.list_locations_admin(&actor).await?;
        Ok(Json(result))
    }

    /// POST /api/admin/locations
    pub async fn create_location(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<CreateLocationRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .create_location(&actor, req.label, req.lat, req.lng, req.display_order)
            .await?;
        Ok((StatusCode::CREATED, Json(result)))
    }

    /// GET /api/admin/locations/{id}
    pub async fn get_location(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.admin.get_location_admin(&actor, id).await?;
        Ok(Json(result))
    }

    /// PUT /api/admin/locations/{id}
    pub async fn update_location(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<UpdateLocationRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .update_location(&actor, id, req.label, req.lat, req.lng, req.display_order)
            .await?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/locations/{id}/status
    pub async fn toggle_location_active(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ToggleActiveRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .toggle_location_active(&actor, id, req.is_active, req.reason)
            .await?;
        Ok(Json(result))
    }

    /// DELETE /api/admin/locations/{id}
    pub async fn delete_location(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        body: Option<ApiJson<DeleteWithReasonBody>>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let reason = body.and_then(|b| b.0.reason);
        state
            .usecases
            .admin
            .delete_location(&actor, id, reason)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    // ── Admin Products ────────────────────────────────────────────────────────

    /// GET /api/admin/products
    pub async fn list_products(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<AdminListQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .list_products_admin(&actor, q.page.unwrap_or(1), q.provider_id)
            .await?;
        Ok(Json(result))
    }

    /// GET /api/admin/products/{id}
    pub async fn get_product(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.admin.get_product_admin(&actor, id).await?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/products/{id}/active
    pub async fn toggle_product_active(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ToggleActiveRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .toggle_product_active(&actor, id, req.is_active, req.reason)
            .await?;
        Ok(Json(result))
    }

    /// DELETE /api/admin/products/{id}
    pub async fn delete_product(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        body: Option<ApiJson<DeleteWithReasonBody>>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let reason = body.and_then(|b| b.0.reason);
        state
            .usecases
            .admin
            .delete_product_admin(&actor, id, reason)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    // ── Admin Services ────────────────────────────────────────────────────────

    /// GET /api/admin/services
    pub async fn list_services(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<AdminListQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .list_services_admin(&actor, q.page.unwrap_or(1), q.provider_id)
            .await?;
        Ok(Json(result))
    }

    /// GET /api/admin/services/{id}
    pub async fn get_service(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.admin.get_service_admin(&actor, id).await?;
        Ok(Json(result))
    }

    /// POST /api/admin/services
    pub async fn create_service(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<AdminCreateServiceRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .create_service_admin(
                &actor,
                req.business_id,
                req.category_id,
                req.name,
                req.description,
                req.price_note,
            )
            .await?;
        Ok((StatusCode::CREATED, Json(result)))
    }

    /// PUT /api/admin/services/{id}
    pub async fn update_service(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<AdminUpdateServiceRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .update_service_admin(
                &actor,
                id,
                req.category_id,
                req.name,
                req.description,
                req.price_note,
            )
            .await?;
        Ok(Json(result))
    }

    /// PATCH /api/admin/services/{id}/active
    pub async fn toggle_service_active(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ToggleActiveRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .toggle_service_active(&actor, id, req.is_active, req.reason)
            .await?;
        Ok(Json(result))
    }

    /// DELETE /api/admin/services/{id}
    pub async fn delete_service(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        body: Option<ApiJson<DeleteWithReasonBody>>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let reason = body.and_then(|b| b.0.reason);
        state
            .usecases
            .admin
            .delete_service_admin(&actor, id, reason)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }
}
