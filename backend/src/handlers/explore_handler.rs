use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use validator::Validate;

use crate::{
    app_state::AppState,
    application::inputs::{BrowseParams, ContactInput, GeoSearchParams},
    constants::RADIUS_DEFAULT_M,
    view_models::{
        requests::{CategoriesQuery, InquiryRequest, ProvidersQuery},
        HandlerError, HandlerResult,
    },
};

pub struct ExploreHandler;

impl ExploreHandler {
    /// GET /api/explore/providers
    /// Geo-search mode when `lat` + `lng` are present; paginated browse otherwise.
    pub async fn list_providers(
        State(state): State<AppState>,
        Query(q): Query<ProvidersQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        q.validate().map_err(|e| HandlerError::validation("Invalid parameters", e))?;

        if let (Some(lat), Some(lng)) = (q.lat, q.lng) {
            let result = state
                .usecases
                .explore
                .search_providers(GeoSearchParams {
                    lat,
                    lng,
                    radius:     q.radius.unwrap_or(RADIUS_DEFAULT_M),
                    service_id: q.service_id,
                })
                .await
                .map_err(HandlerError::from)?;
            Ok(Json(serde_json::to_value(result).unwrap()))
        } else {
            let result = state
                .usecases
                .explore
                .browse_providers(BrowseParams {
                    service_id: q.service_id,
                    name:       q.name,
                    sort:       q.sort,
                    page:       q.page.unwrap_or(1),
                })
                .await
                .map_err(HandlerError::from)?;
            Ok(Json(serde_json::to_value(result).unwrap()))
        }
    }

    /// GET /api/explore/providers/:id
    pub async fn get_provider_by_id(
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .explore
            .get_provider_detail(id)
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// GET /api/explore/featured
    pub async fn get_explore_data(
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .explore
            .get_explore_data()
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// GET /api/explore/categories?type=product|service
    pub async fn get_categories(
        State(state): State<AppState>,
        Query(q): Query<CategoriesQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .explore
            .get_categories(q.cat_type.as_deref())
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// GET /api/explore/locations
    pub async fn get_locations(
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .explore
            .get_locations()
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// POST /api/explore/contact
    pub async fn contact_request(
        State(state): State<AppState>,
        Json(req): Json<InquiryRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        req.validate().map_err(|e| HandlerError::validation("Invalid request data", e))?;

        let result = state
            .usecases
            .explore
            .contact_request(ContactInput {
                business_id:     req.business_id,
                topic:           req.topic,
                requester_name:  req.requester_name,
                requester_phone: req.requester_phone,
                note:            req.note,
                service_id:      req.service_id,
            })
            .await
            .map_err(HandlerError::from)?;

        Ok((StatusCode::CREATED, Json(result)))
    }
}
