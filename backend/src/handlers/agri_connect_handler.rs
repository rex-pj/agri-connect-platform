use axum::{
    extract::{Path, Query, State},
    Json,
};
use validator::Validate;

use crate::{
    app_state::AppState,
    application::inputs::{BrowseParams, ContactInput, GeoSearchParams},
    constants::RADIUS_DEFAULT_M,
    view_models::{
        requests::{BrowseQuery, CategoriesQuery, InquiryRequest, SearchQuery},
        HandlerError, HandlerResult,
    },
};

pub struct AgriConnectHandler;

impl AgriConnectHandler {
    /// GET /api/agri-connect/providers?lat=&lng=&radius=&service=
    pub async fn search_providers(
        State(state): State<AppState>,
        Query(q): Query<SearchQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        q.validate().map_err(|e| HandlerError::validation("Invalid search parameters", e))?;

        let result = state
            .usecases
            .agri_connect
            .search_providers(GeoSearchParams {
                lat:        q.lat,
                lng:        q.lng,
                radius:     q.radius.unwrap_or(RADIUS_DEFAULT_M),
                service_id: q.service_id,
            })
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// GET /api/agri-connect/providers/browse?service=&page=
    pub async fn browse_providers(
        State(state): State<AppState>,
        Query(q): Query<BrowseQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        q.validate().map_err(|e| HandlerError::validation("Invalid parameters", e))?;

        let result = state
            .usecases
            .agri_connect
            .browse_providers(BrowseParams {
                service_id: q.service_id,
                name:       q.name,
                sort:       q.sort,
                page:       q.page.unwrap_or(1),
            })
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// GET /api/agri-connect/providers/:id
    pub async fn get_provider_by_id(
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .agri_connect
            .get_provider_detail(id)
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// GET /api/agri-connect/explore
    pub async fn get_explore_data(
        State(state): State<AppState>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .agri_connect
            .get_explore_data()
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// GET /api/agri-connect/categories?type=product|service
    pub async fn get_categories(
        State(state): State<AppState>,
        Query(q): Query<CategoriesQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .agri_connect
            .get_categories(q.cat_type.as_deref())
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// POST /api/agri-connect/contact
    pub async fn contact_request(
        State(state): State<AppState>,
        Json(req): Json<InquiryRequest>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        req.validate().map_err(|e| HandlerError::validation("Invalid request data", e))?;

        state
            .usecases
            .agri_connect
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

        Ok(Json(serde_json::json!({
            "message": "Your enquiry has been recorded. The provider will get back to you."
        })))
    }
}
