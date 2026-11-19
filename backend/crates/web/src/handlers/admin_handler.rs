use application::authz::require_actor;
use application::inputs::{CategoryInput, CreateProductInput, UpdateProductInput};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath, ApiQuery},
    labels::Worded,
    middleware::auth::MaybeActor,
    requests::{
        AdminCreateProductRequest, AdminCreateServiceRequest, AdminListQuery, AdminPageQuery,
        AdminUpdateProductRequest, AdminUpdateServiceRequest, AssignUserRequest,
        CreateCategoryRequest, DeleteWithReasonBody, LocationRequest, SetVerificationRequest,
        SlugSuggestionQuery, StatusListQuery, ToggleActiveRequest, ToggleVisibleRequest,
        UpdateCategoryRequest, UpdateInquiryStatusRequest, UpdateProfileRequest,
        UpdateUserRoleRequest,
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
            .list_providers(&actor, q.page.unwrap_or(1), q.name.as_deref())
            .await?;
        Ok(Json(result))
    }

    /// POST /api/admin/providers — a listing with no account yet (the same
    /// boxes as the owner's profile); 201 with its details.
    pub async fn create_provider(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<UpdateProfileRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let admin = &state.usecases.admin;
        let id = admin.create_provider(&actor, req.into()).await?;
        let result = admin.get_provider_detail(&actor, id).await?;
        Ok((StatusCode::CREATED, Json(result)))
    }

    /// PUT /api/admin/providers/{id} — replaces the profile; its details.
    pub async fn update_provider(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<UpdateProfileRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let admin = &state.usecases.admin;
        admin.update_provider(&actor, id, req.into()).await?;
        let result = admin.get_provider_detail(&actor, id).await?;
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

    /// PATCH /api/admin/providers/{id}/status
    pub async fn toggle_provider_active(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ToggleActiveRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .toggle_provider_active(&actor, id, req.is_active, req.reason)
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
        Ok(Json(result.worded(state.locale())))
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
            .list_users(&actor, q.page.unwrap_or(1), q.name.as_deref())
            .await?;
        Ok(Json(result.worded(state.locale())))
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
        Ok(Json(result.worded(state.locale())))
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
                    sector: req.sector,
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
                    sector: req.sector,
                },
            )
            .await?;
        Ok(Json(result))
    }

    /// GET /api/admin/categories/slug: a free slug for the category form.
    pub async fn suggest_category_slug(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<SlugSuggestionQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .suggest_category_slug(&actor, &q.text, q.id)
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
        ApiQuery(q): ApiQuery<StatusListQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .list_inquiries(
                &actor,
                q.page.unwrap_or(1),
                q.provider_id,
                q.status.as_deref(),
            )
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
            .update_inquiry_status(&actor, id, req.status, req.reason)
            .await?;
        Ok(Json(result))
    }

    /// GET /api/admin/reviews
    pub async fn list_reviews(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<AdminListQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .list_reviews(&actor, q.page.unwrap_or(1), q.provider_id)
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
        ApiJson(req): ApiJson<LocationRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .create_location(&actor, req.into())
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
        ApiJson(req): ApiJson<LocationRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .update_location(&actor, id, req.into())
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
            .list_products_admin(
                &actor,
                q.page.unwrap_or(1),
                &domain::models::product::AdminProductFilter {
                    provider_id: q.provider_id,
                    search: q.name,
                    ..Default::default()
                },
            )
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

    /// POST /api/admin/products
    pub async fn create_product(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<AdminCreateProductRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let p = req.product;
        let result = state
            .usecases
            .admin
            .create_product_admin(
                &actor,
                req.business_id,
                CreateProductInput {
                    category_id: p.category_id,
                    commodity_id: p.commodity_id,
                    name: p.name,
                    description: p.description,
                    price: p.price,
                    unit: p.unit,
                },
            )
            .await?;
        Ok((StatusCode::CREATED, Json(result)))
    }

    /// PUT /api/admin/products/{id}
    pub async fn update_product(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<AdminUpdateProductRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .admin
            .update_product_admin(
                &actor,
                UpdateProductInput {
                    product_id: id,
                    category_id: req.category_id,
                    commodity_id: req.commodity_id,
                    name: req.name,
                    description: req.description,
                    price: req.price,
                    unit: req.unit,
                },
            )
            .await?;
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
            .list_services_admin(
                &actor,
                q.page.unwrap_or(1),
                q.provider_id,
                q.name.as_deref(),
            )
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
