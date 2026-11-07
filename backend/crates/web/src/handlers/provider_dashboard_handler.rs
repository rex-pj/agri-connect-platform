use application::authz::require_actor;
use application::inputs::{CreateProductInput, UpdateProductInput};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::Error;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath},
    middleware::auth::MaybeActor,
    requests::{
        CreateProductRequest, OwnServiceRequest, ReviewReplyRequest, UpdateProductRequest,
        UpdateProfileRequest,
    },
};

pub struct ProviderDashboardHandler;

impl ProviderDashboardHandler {
    /// GET /api/provider/profile
    pub async fn get_profile(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .provider_dashboard
            .get_profile(&actor)
            .await?;

        Ok(Json(result))
    }

    /// PUT /api/provider/profile
    pub async fn update_profile(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<UpdateProfileRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .provider_dashboard
            .update_profile(&actor, req.into())
            .await?;

        Ok(Json(result))
    }

    /// GET /api/provider/reviews — every review of the own business, shown or
    /// waiting for an admin.
    pub async fn list_reviews(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state.usecases.review.list_own_reviews(&actor).await?;
        Ok(Json(result))
    }

    /// PUT /api/provider/reviews/{id}/reply — the public answer; 204.
    pub async fn reply_review(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<ReviewReplyRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .review
            .reply_own_review(&actor, id, req.reply)
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    /// GET /api/provider/products
    pub async fn list_products(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .provider_dashboard
            .list_products(&actor)
            .await?;

        Ok(Json(result))
    }

    /// POST /api/provider/products
    pub async fn create_product(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<CreateProductRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .provider_dashboard
            .create_product(
                &actor,
                CreateProductInput {
                    category_id: req.category_id,
                    name: req.name,
                    description: req.description,
                    price: req.price,
                    unit: req.unit,
                },
                // The JSON API sends the photo separately (PUT …/image).
                None,
            )
            .await?;

        Ok((StatusCode::CREATED, Json(result)))
    }

    /// PUT /api/provider/products/{id}
    pub async fn update_product(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<UpdateProductRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .provider_dashboard
            .update_product(
                &actor,
                UpdateProductInput {
                    product_id: id,
                    category_id: req.category_id,
                    name: req.name,
                    description: req.description,
                    price: req.price,
                    unit: req.unit,
                },
            )
            .await?;

        Ok(Json(result))
    }

    /// DELETE /api/provider/products/{id}
    pub async fn delete_product(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .provider_dashboard
            .delete_product(&actor, id)
            .await?;

        Ok(StatusCode::NO_CONTENT)
    }

    /// GET /api/provider/services
    pub async fn list_services(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .provider_dashboard
            .list_services(&actor)
            .await?;

        Ok(Json(result))
    }

    /// POST /api/provider/services
    pub async fn create_service(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<OwnServiceRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .provider_dashboard
            // The JSON API sends the photo separately (PUT …/image).
            .create_service(&actor, req.into_input(), None)
            .await?;

        Ok((StatusCode::CREATED, Json(result)))
    }

    /// PUT /api/provider/services/{id}
    pub async fn update_service(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        ApiJson(req): ApiJson<OwnServiceRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        let result = state
            .usecases
            .provider_dashboard
            .update_service(&actor, id, req.into_input())
            .await?;

        Ok(Json(result))
    }

    /// DELETE /api/provider/services/{id}
    pub async fn delete_service(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        state
            .usecases
            .provider_dashboard
            .delete_service(&actor, id)
            .await?;

        Ok(StatusCode::NO_CONTENT)
    }
}
