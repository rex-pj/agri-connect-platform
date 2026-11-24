use application::{
    catalogue::PERM_FEATURE_PREVIEW,
    constants::RADIUS_DEFAULT_M,
    inputs::{
        BrowseParams, CallInput, ContactInput, GeoSearchParams, SearchInput, SearchPlace, Visitor,
    },
    outputs::{BrowseResponse, BusinessSearchDto, PublicServiceDto, ServiceBrowseResponse},
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::{models::business::ProviderSort, Error};
use serde::Serialize;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiPath, ApiQuery},
    middleware::{
        auth::Viewer,
        client_ip::{client_key, ClientIp},
    },
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

/// Who is asking, for the counts that tell visitors apart: anyone signed
/// in is not demand; anyone else is their address (`client_key`, in memory
/// only). A stale session cookie is nobody (`Viewer`).
fn visitor_of(viewer: &Viewer, ip: Option<&ClientIp>) -> Visitor {
    match viewer.0 {
        Some(_) => Visitor::SignedIn,
        None => Visitor::Anonymous {
            key: client_key(ip),
        },
    }
}

/// A near-me search of the JSON API (the Zalo Mini App's): demand in that
/// area, found or not, recorded like the near-me page's. Asking for the
/// providers and the services of one place is one search (the repeat
/// filter), as is asking again.
fn record_near(state: &AppState, geo: &GeoSearchParams, results: usize, visitor: Visitor) {
    state.usecases.demand.record_search(SearchInput {
        query: String::new(),
        category_id: geo.service_id,
        place: SearchPlace::Point {
            lat: geo.lat,
            lng: geo.lng,
        },
        results: i64::try_from(results).unwrap_or(i64::MAX),
        visitor,
    });
}

pub struct ExploreHandler;

impl ExploreHandler {
    /// GET /api/explore/providers
    /// Geo-search mode when `lat` + `lng` are present; paginated browse otherwise.
    pub async fn list_providers(
        viewer: Viewer,
        ip: Option<axum::Extension<ClientIp>>,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<ProvidersQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let explore = &state.usecases.explore;
        Ok(Json(match geo_params(&q) {
            Some(geo) => {
                let found = explore.search_providers(geo.clone()).await?;
                record_near(
                    &state,
                    &geo,
                    found.len(),
                    visitor_of(&viewer, ip.as_deref()),
                );
                ProvidersResponse::Nearby(found)
            }
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
        viewer: Viewer,
        ip: Option<axum::Extension<ClientIp>>,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<ProvidersQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let explore = &state.usecases.explore;
        Ok(Json(match geo_params(&q) {
            Some(geo) => {
                let found = explore.search_services(geo.clone()).await?;
                record_near(
                    &state,
                    &geo,
                    found.len(),
                    visitor_of(&viewer, ip.as_deref()),
                );
                ServicesResponse::Nearby(found)
            }
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

    /// GET /api/explore/features — which parts of the platform the caller
    /// sees (`{ "prices": true, … }` by `Feature::code`): the Zalo Mini App
    /// shows a tab only for one that is on. A part in preview is shown
    /// only to a caller with `feature.preview`.
    pub async fn features(
        Viewer(viewer): Viewer,
        State(state): State<AppState>,
    ) -> Result<impl IntoResponse, Error> {
        let may_preview = viewer.is_some_and(|a| a.can(PERM_FEATURE_PREVIEW));
        let settings = state.usecases.settings.current().await?;
        Ok(Json(settings.features_shown(may_preview)))
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
    /// call button was tapped; 204. Rate-limited like the contact form. A
    /// stale session cookie is nobody (`Viewer`): the tap still counts.
    pub async fn record_call(
        viewer: Viewer,
        ip: Option<axum::Extension<ClientIp>>,
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
                // One visitor's repeated taps are one lead.
                visitor: visitor_of(&viewer, ip.as_deref()),
            })
            .await?;
        Ok(StatusCode::NO_CONTENT)
    }

    /// POST /api/explore/contact
    pub async fn contact_request(
        ip: Option<axum::Extension<ClientIp>>,
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
                client: client_key(ip.as_deref()),
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
