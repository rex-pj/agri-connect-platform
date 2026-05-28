use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    app_state::AppState,
    application::inputs::{CreateProductInput, UpdateProductInput, UpdateProfileInput},
    middleware::auth::RequireProviderOrAdmin,
    view_models::{
        requests::{CreateProductRequest, UpdateProductRequest, UpdateProfileRequest},
        HandlerError, HandlerResult,
    },
};

pub struct ProviderDashboardHandler;

impl ProviderDashboardHandler {
    /// GET /api/provider/profile
    pub async fn get_profile(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .provider_dashboard
            .get_profile(caller.id)
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// PUT /api/provider/profile
    pub async fn update_profile(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
        Json(req): Json<UpdateProfileRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .provider_dashboard
            .update_profile(
                &caller.role,
                caller.id,
                UpdateProfileInput {
                    user_id:     caller.id,
                    name:        req.name,
                    phone:       req.phone,
                    description: req.description,
                    address:     req.address,
                    lat:         req.lat,
                    lng:         req.lng,
                },
            )
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// GET /api/provider/products
    pub async fn list_products(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .provider_dashboard
            .list_products(caller.id)
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// POST /api/provider/products
    pub async fn create_product(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
        Json(req): Json<CreateProductRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .provider_dashboard
            .create_product(
                &caller.role,
                caller.id,
                CreateProductInput {
                    user_id:     caller.id,
                    category_id: req.category_id,
                    name:        req.name,
                    description: req.description,
                    price:       req.price,
                    unit:        req.unit,
                    image_url:   req.image_url,
                },
            )
            .await
            .map_err(HandlerError::from)?;

        Ok((StatusCode::CREATED, Json(result)))
    }

    /// PUT /api/provider/products/{id}
    pub async fn update_product(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
        Json(req): Json<UpdateProductRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .provider_dashboard
            .update_product(
                &caller.role,
                caller.id,
                UpdateProductInput {
                    product_id:  id,
                    user_id:     caller.id,
                    category_id: req.category_id,
                    name:        req.name,
                    description: req.description,
                    price:       req.price,
                    unit:        req.unit,
                    image_url:   req.image_url,
                },
            )
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// DELETE /api/provider/products/{id}
    pub async fn delete_product(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        state
            .usecases
            .provider_dashboard
            .delete_product(&caller.role, caller.id, id)
            .await
            .map_err(HandlerError::from)?;

        Ok(StatusCode::NO_CONTENT)
    }
}
