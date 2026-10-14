use application::inputs::{CreateProductInput, UpdateProductInput, UpdateProfileInput};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath},
    middleware::auth::RequireProviderOrAdmin,
    requests::{CreateProductRequest, UpdateProductRequest, UpdateProfileRequest},
};

pub struct ProviderDashboardHandler;

impl ProviderDashboardHandler {
    /// GET /api/provider/profile
    pub async fn get_profile(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state
            .usecases
            .provider_dashboard
            .get_profile(caller.id)
            .await?;

        Ok(Json(result))
    }

    /// PUT /api/provider/profile
    pub async fn update_profile(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<UpdateProfileRequest>,
    ) -> Result<impl IntoResponse, Error> {
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
            .await?;

        Ok(Json(result))
    }

    /// GET /api/provider/products
    pub async fn list_products(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state
            .usecases
            .provider_dashboard
            .list_products(caller.id)
            .await?;

        Ok(Json(result))
    }

    /// POST /api/provider/products
    pub async fn create_product(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<CreateProductRequest>,
    ) -> Result<impl IntoResponse, Error> {
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
                },
            )
            .await?;

        Ok((StatusCode::CREATED, Json(result)))
    }

    /// PUT /api/provider/products/{id}
    pub async fn update_product(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<UpdateProductRequest>,
    ) -> Result<impl IntoResponse, Error> {
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
                },
            )
            .await?;

        Ok(Json(result))
    }

    /// DELETE /api/provider/products/{id}
    pub async fn delete_product(
        RequireProviderOrAdmin(caller): RequireProviderOrAdmin,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        state
            .usecases
            .provider_dashboard
            .delete_product(&caller.role, caller.id, id)
            .await?;

        Ok(StatusCode::NO_CONTENT)
    }
}
