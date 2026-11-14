use application::{
    constants::RADIUS_DEFAULT_M,
    inputs::{BrowseParams, CallInput, ContactInput, GeoSearchParams, Visitor},
    outputs::{BrowseResponse, BusinessSearchDto, PublicServiceDto, ServiceBrowseResponse},
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::{models::business::ProviderSort, Error};
use serde::Serialize;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath, ApiQuery},
    middleware::auth::MaybeActor,
    pages::listing::list_sector,
    requests::{
        CallRequest, CategoriesQuery, InquiryRequest, ProvidersQuery, ReviewRequest, SectorQuery,
    },
};

/// `/api/explore/providers` answers in two shapes depending on whether a
/// location was given; clients tell them apart by the top-level JSON type.
#[derive(Serialize)]
#[serde(untagged)]
enum ProvidersResponse {
    Nearby(Vec<BusinessSearchDto>),
    Browse(BrowseResponse),
}

/// `/api/explore/services`: the same two shapes as the providers endpoint.
#[derive(Serialize)]
#[serde(untagged)]
enum ServicesResponse {
    Nearby(Vec<PublicServiceDto>),
    Browse(ServiceBrowseResponse),
}

/// The near-me search, when both coordinates were given.
fn geo_params(q: &ProvidersQuery) -> Option<GeoSearchParams> {
    let (Some(lat), Some(lng)) = (q.lat, q.lng) else {
        return None;
    };
    Some(GeoSearchParams {
        lat,
        lng,
        radius: q.radius.unwrap_or(RADIUS_DEFAULT_M),
        service_id: q.service_id,
    })
}

async fn browse_params(state: &AppState, q: ProvidersQuery) -> Result<BrowseParams, Error> {
    let sector = state
        .usecases
        .sector
        .chosen(q.sector.as_deref().unwrap_or_default())
        .await?;
    let name = q.name.filter(|n| !n.trim().is_empty());
    Ok(BrowseParams {
        sector: list_sector(
            sector.as_deref(),
            q.service_id,
            name.as_deref().unwrap_or_default(),
        ),
        service_id: q.service_id,
        name,
        sort: ProviderSort::parse(q.sort.as_deref()),
        page: q.page.unwrap_or(1),
    })
}

pub struct ExploreHandler;

impl ExploreHandler {
    /// GET /api/explore/providers
    /// Geo-search mode when `lat` + `lng` are present; paginated browse otherwise.
    pub async fn list_providers(
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<ProvidersQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let explore = &state.usecases.explore;
        Ok(Json(match geo_params(&q) {
            Some(geo) => ProvidersResponse::Nearby(explore.search_providers(geo).await?),
            None => ProvidersResponse::Browse(
                explore
                    .browse_providers(browse_params(&state, q).await?)
                    .await?,
            ),
        }))
    }

    /// GET /api/explore/services
    /// Near-me mode when `lat` + `lng` are present; paginated browse otherwise
    /// (`category_id`, `name` as the typed text, `sort`, `page`).
    pub async fn list_services(
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<ProvidersQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let explore = &state.usecases.explore;
        Ok(Json(match geo_params(&q) {
            Some(geo) => ServicesResponse::Nearby(explore.search_services(geo).await?),
            None => ServicesResponse::Browse(
                explore
                    .browse_services(browse_params(&state, q).await?)
                    .await?,
            ),
        }))
    }

    /// GET /api/explore/services/{id}
    pub async fn get_service_by_id(
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        Ok(Json(state.usecases.explore.get_service(id).await?))
    }

    /// GET /api/explore/providers/:id
    pub async fn get_provider_by_id(
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state.usecases.explore.get_provider_detail(id).await?;

        Ok(Json(result))
    }

    /// GET /api/explore/featured?sector=
    pub async fn get_explore_data(
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<SectorQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let sector = state.usecases.sector.chosen(&q.sector).await?;
        Ok(Json(
            state
                .usecases
                .explore
                .get_explore_data(sector.as_deref())
                .await?,
        ))
    }

    /// GET /api/explore/alerts?sector= — the alerts in force today for that
    /// sector (and every sector); with none, all of them, as the home page.
    pub async fn list_alerts(
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<SectorQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let sector = state.usecases.sector.chosen(&q.sector).await?;
        Ok(Json(state.usecases.alert.live(sector.as_deref()).await?))
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

    /// POST /api/explore/calls — `{ kind, id, channel? }` of the card or page whose
    /// call button was tapped; 204. Rate-limited like the contact form.
    pub async fn record_call(
        MaybeActor(actor): MaybeActor,
        ip: Option<axum::Extension<crate::middleware::client_ip::ClientIp>>,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<CallRequest>,
    ) -> Result<impl IntoResponse, Error> {
        state
            .usecases
            .explore
            .record_call(CallInput {
                kind: req.kind,
                id: req.id,
                channel: req.channel,
                visitor: if actor.is_some() {
                    Visitor::SignedIn
                } else {
                    Visitor::Anonymous {
                        // Told apart by address (in memory only), so one
                        // visitor's repeated taps are one lead.
                        key: ip.map(|i| i.0 .0.to_string()).unwrap_or_default(),
                    }
                },
            })
            .await?;
        Ok(StatusCode::NO_CONTENT)
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
                product_id: req.product_id,
            })
            .await?;

        Ok((StatusCode::CREATED, Json(result)))
    }

    /// GET /api/explore/providers/{id}/reviews — the shown reviews: count,
    /// average and the newest.
    pub async fn list_reviews(
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state.usecases.review.list_public(id).await?;
        Ok(Json(result))
    }

    /// POST /api/explore/reviews — 201; the review waits for an admin.
    /// Rate-limited like the contact form.
    pub async fn submit_review(
        State(state): State<AppState>,
        ApiJson(req): ApiJson<ReviewRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state.usecases.review.submit(req.into()).await?;
        Ok((StatusCode::CREATED, Json(result)))
    }
}
