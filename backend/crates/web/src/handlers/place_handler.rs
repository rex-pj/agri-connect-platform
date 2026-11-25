//! Where a business is, for the location box: the ward a dropped pin lies
//! in, and the map's tiles. Public and sessionless: the wards and the map
//! are public knowledge, and a tile is an asset (no session is read).

use axum::{
    extract::State,
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use domain::Error;

use crate::{
    app_state::AppState,
    constants::MAP_TILE_CACHE_SECS,
    extract::{ApiPath, ApiQuery},
    requests::{PlaceQuery, WardSearchQuery},
};

pub struct PlaceHandler;

impl PlaceHandler {
    /// GET /api/places/ward?lat=&lng= — `{area, label, approximate}`; `area`
    /// empty outside every ward.
    pub async fn ward(
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<PlaceQuery>,
    ) -> Result<impl IntoResponse, Error> {
        Ok(Json(state.usecases.place.ward_at(q.lat, q.lng).await?))
    }

    /// GET /api/places/wards?q= — `{wards: [{value, label}]}`, the wards
    /// whose name or province holds every word, by name.
    pub async fn wards(
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<WardSearchQuery>,
    ) -> Result<impl IntoResponse, Error> {
        Ok(Json(state.usecases.place.search_wards(&q.q).await?))
    }

    /// GET /map/tiles/{z}/{x}/{y} — a PNG tile of the region, kept a week by
    /// the browser and the CDN.
    pub async fn tile(
        State(state): State<AppState>,
        ApiPath((z, x, y)): ApiPath<(u8, u32, u32)>,
    ) -> Result<Response, Error> {
        let png = state.usecases.place.tile(z, x, y).await?;
        let cache = HeaderValue::from_str(&format!("public, max-age={MAP_TILE_CACHE_SECS}"))
            .map_err(|e| Error::internal(format!("tile cache header: {e}")))?;
        Ok((
            [
                (header::CONTENT_TYPE, HeaderValue::from_static("image/png")),
                (header::CACHE_CONTROL, cache),
            ],
            png,
        )
            .into_response())
    }
}
