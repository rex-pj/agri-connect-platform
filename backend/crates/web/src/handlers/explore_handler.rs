use application::{
    constants::RADIUS_DEFAULT_M,
    inputs::{BrowseParams, ContactInput, GeoSearchParams},
    outputs::{BrowseResponse, BusinessSearchDto},
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::{models::business::ProviderSort, Error};
use serde::Serialize;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath, ApiQuery},
    requests::{CategoriesQuery, InquiryRequest, ProvidersQuery},
};

/// `/api/explore/providers` answers in two shapes depending on whether a
/// location was given; clients tell them apart by the top-level JSON type.
#[derive(Serialize)]
#[serde(untagged)]
enum ProvidersResponse {
    Nearby(Vec<BusinessSearchDto>),
    Browse(BrowseResponse),
}

pub struct ExploreHandler;

impl ExploreHandler {
    /// GET /api/explore/providers
    /// Geo-search mode when `lat` + `lng` are present; paginated browse otherwise.
    pub async fn list_providers(
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<ProvidersQuery>,
    ) -> Result<impl IntoResponse, Error> {
        if let (Some(lat), Some(lng)) = (q.lat, q.lng) {
            let result = state
                .usecases
                .explore
                .search_providers(GeoSearchParams {
                    lat,
                    lng,
                    radius: q.radius.unwrap_or(RADIUS_DEFAULT_M),
                    service_id: q.service_id,
                })
                .await?;
            Ok(Json(ProvidersResponse::Nearby(result)))
        } else {
            let result = state
                .usecases
                .explore
                .browse_providers(BrowseParams {
                    service_id: q.service_id,
                    name: q.name,
                    sort: ProviderSort::parse(q.sort.as_deref()),
                    page: q.page.unwrap_or(1),
                })
                .await?;
            Ok(Json(ProvidersResponse::Browse(result)))
        }
    }

    /// GET /api/explore/providers/:id
    pub async fn get_provider_by_id(
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state.usecases.explore.get_provider_detail(id).await?;

        Ok(Json(result))
    }

    /// GET /api/explore/featured
    pub async fn get_explore_data(
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state.usecases.explore.get_explore_data().await?;

        Ok(Json(result))
    }

    /// GET /api/explore/categories?type=product|service
    pub async fn get_categories(
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<CategoriesQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state
            .usecases
            .explore
            .get_categories(q.cat_type.as_deref())
            .await?;

        Ok(Json(result))
    }

    /// GET /api/explore/locations
    pub async fn get_locations(State(state): State<AppState>) -> Result<impl IntoResponse, Error> {
        let result = state.usecases.explore.get_locations().await?;

        Ok(Json(result))
    }

    /// POST /api/explore/contact
    pub async fn contact_request(
        State(state): State<AppState>,
        ApiJson(req): ApiJson<InquiryRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state
            .usecases
            .explore
            .contact_request(ContactInput {
                business_id: req.business_id,
                topic: req.topic,
                requester_name: req.requester_name,
                requester_phone: req.requester_phone,
                note: req.note,
                service_id: req.service_id,
            })
            .await?;

        Ok((StatusCode::CREATED, Json(result)))
    }
}
