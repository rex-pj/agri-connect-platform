//! GET /explore — services, products and providers near the visitor, in three
//! tabs (`tab=providers`, `tab=products`; links, so they work without JS).
//! Before a position is known it features services, like the home page.
//!
//! The position comes from the `ac-geolocate` island (`lat`, `lng`) or, with
//! or without JS, from a location preset chosen in a GET form
//! (`location_id`). Service and radius filters are links that keep the
//! position. Coordinates are in the query string but never in the logs: the
//! request span records the path only.

use application::outputs::{CategoryDto, LocationDto, ProductDto};
use application::{
    constants::{LAT_MAX, LAT_MIN, LNG_MAX, LNG_MIN},
    inputs::GeoSearchParams,
};
use axum::{extract::State, response::Response};
use domain::{constants::CATEGORY_TYPE_SERVICE, Error};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::{EXPLORE_DEFAULT_RADIUS_M, EXPLORE_RADII_M},
    extract::ApiQuery,
    pages::{
        cards::{tile, CategoryTile, ProviderCard, ServiceCard},
        listing::{parse_opt, with_query},
        search::TabLink,
    },
    routes,
    shell::Shell,
    views::render_page,
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ExploreQuery {
    pub lat: String,
    pub lng: String,
    pub location_id: String,
    pub radius: String,
    pub service_id: String,
    /// `providers` for the shops tab; anything else shows the services.
    pub tab: String,
}

/// Where the search is centred.
#[derive(Debug, Clone)]
enum Position {
    /// From the device; kept as the visitor's own (rounded) text.
    Device {
        lat: f64,
        lng: f64,
        lat_text: String,
        lng_text: String,
    },
    Preset(LocationDto),
}

impl Position {
    fn coordinates(&self) -> (f64, f64) {
        match self {
            Position::Device { lat, lng, .. } => (*lat, *lng),
            Position::Preset(l) => (l.lat, l.lng),
        }
    }

    /// The query parameters that reproduce this position.
    fn params(&self) -> Vec<(&'static str, String)> {
        match self {
            Position::Device {
                lat_text, lng_text, ..
            } => {
                vec![("lat", lat_text.clone()), ("lng", lng_text.clone())]
            }
            Position::Preset(l) => vec![("location_id", l.id.to_string())],
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RadiusLink {
    pub label: String,
    pub href: String,
    pub active: bool,
}

#[derive(Serialize)]
pub struct ExplorePage {
    /// A position is known: results instead of the onboarding.
    pub located: bool,
    /// The preset's name, when the position is a preset.
    pub place: Option<String>,
    /// Where the island goes once it has the device position.
    pub geolocate_href: String,
    pub locations: Vec<LocationDto>,
    pub location_id: Option<i32>,
    pub radius_km: i64,
    pub radius_links: Vec<RadiusLink>,
    /// The next larger radius, offered when nothing is found.
    pub wider: Option<RadiusLink>,
    pub service_tiles: Vec<CategoryTile>,
    pub service_id: Option<i32>,
    /// The results tab shown without JS: `services`, `providers` or `products`.
    pub tab: &'static str,
    pub tabs: Vec<TabLink>,
    /// Services whose provider is within the radius, nearest first.
    pub services: Vec<ServiceCard>,
    pub providers: Vec<ProviderCard>,
    /// Products whose seller is within the radius, nearest first.
    pub products: Vec<ProductDto>,
    /// Before a position is known: the same featured services as the home page.
    pub featured_services: Vec<ServiceCard>,
}

const TAB_SERVICES: &str = "services";
pub(super) const TAB_PROVIDERS: &str = "providers";
const TAB_PRODUCTS: &str = "products";

fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("explore links: {e}")))
}

fn icon(c: &CategoryDto) -> &str {
    c.icon.as_deref().unwrap_or("fa-tag")
}

/// Device coordinates, if both parse and are on the globe. Anything else is
/// "no position yet", not an error: the link may be hand-edited or old.
fn device_position(q: &ExploreQuery) -> Option<Position> {
    let lat: f64 = parse_opt(&q.lat)?;
    let lng: f64 = parse_opt(&q.lng)?;
    // The use case's own bounds, so a position we accept is one it accepts.
    let on_globe = (LAT_MIN..=LAT_MAX).contains(&lat) && (LNG_MIN..=LNG_MAX).contains(&lng);
    on_globe.then(|| Position::Device {
        lat,
        lng,
        lat_text: q.lat.trim().to_owned(),
        lng_text: q.lng.trim().to_owned(),
    })
}

pub async fn show(
    shell: Shell,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ExploreQuery>,
) -> Result<Response, Error> {
    let explore = &state.usecases.explore;
    let (service_categories, locations) = tokio::try_join!(
        explore.get_categories(Some(CATEGORY_TYPE_SERVICE)),
        explore.get_locations(),
    )?;

    let position = device_position(&q).or_else(|| {
        let id: i32 = parse_opt(&q.location_id)?;
        locations
            .iter()
            .find(|l| l.id == id)
            .cloned()
            .map(Position::Preset)
    });
    let radius = parse_opt::<i64>(&q.radius)
        .filter(|r| EXPLORE_RADII_M.contains(r))
        .unwrap_or(EXPLORE_DEFAULT_RADIUS_M);
    let service_id =
        parse_opt::<i32>(&q.service_id).filter(|id| service_categories.iter().any(|c| c.id == *id));

    let tab = [TAB_PROVIDERS, TAB_PRODUCTS]
        .into_iter()
        .find(|t| *t == q.tab.trim())
        .unwrap_or(TAB_SERVICES);

    let path = url("explore", &[])?;
    // A link to this page with the position kept and the given filters and tab.
    let link_to = |radius: i64, service: Option<i32>, tab: &str| {
        let mut params: Vec<(&str, String)> =
            position.as_ref().map(Position::params).unwrap_or_default();
        if radius != EXPLORE_DEFAULT_RADIUS_M {
            params.push(("radius", radius.to_string()));
        }
        if let Some(id) = service {
            params.push(("service_id", id.to_string()));
        }
        if tab != TAB_SERVICES {
            params.push(("tab", tab.to_owned()));
        }
        let borrowed: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        with_query(&path, &borrowed)
    };
    let link = |radius: i64, service: Option<i32>| link_to(radius, service, tab);

    let (services, providers, products, featured_services) = match &position {
        // No position yet: what the home page features, instead of results.
        None => (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            ServiceCard::all(explore.get_explore_data().await?.featured_services)?,
        ),
        Some(p) => {
            let (lat, lng) = p.coordinates();
            let params = || GeoSearchParams {
                lat,
                lng,
                radius: radius as f64,
                service_id,
            };
            let (services, providers, products) = tokio::try_join!(
                explore.search_services(params()),
                explore.search_providers(params()),
                explore.search_products(params()),
            )?;
            (
                ServiceCard::all(services)?,
                providers.into_iter().map(ProviderCard::from).collect(),
                products,
                Vec::new(),
            )
        }
    };
    let tabs = [
        // The order of the bottom tabs and the search page's.
        (TAB_SERVICES, "nav.services", services.len()),
        (TAB_PRODUCTS, "nav.products", products.len()),
        (TAB_PROVIDERS, "nav.shops", providers.len()),
    ]
    .into_iter()
    .map(|(t, label, count)| TabLink {
        tab: t,
        label: state.locale().text(label),
        href: link_to(radius, service_id, t),
        count: Some(count as i64),
        active: t == tab,
    })
    .collect();

    let mut service_tiles = Vec::with_capacity(service_categories.len());
    for c in &service_categories {
        let active = service_id == Some(c.id);
        let href = if position.is_some() {
            // Tapping the chosen service again clears the filter.
            link(radius, if active { None } else { Some(c.id) })
        } else {
            url(
                "service_category",
                &[("id", &c.id.to_string()), ("slug", &c.slug)],
            )?
        };
        let mut t = tile(&c.name, icon(c), href);
        t.active = active;
        service_tiles.push(t);
    }
    let page = ExplorePage {
        located: position.is_some(),
        place: match &position {
            Some(Position::Preset(l)) => Some(l.label.clone()),
            _ => None,
        },
        geolocate_href: link(radius, service_id),
        location_id: match &position {
            Some(Position::Preset(l)) => Some(l.id),
            _ => None,
        },
        locations,
        radius_km: radius / 1000,
        wider: EXPLORE_RADII_M
            .iter()
            .find(|&&r| r > radius)
            .map(|&r| RadiusLink {
                label: format!("{} km", r / 1000),
                href: link(r, service_id),
                active: false,
            }),
        radius_links: EXPLORE_RADII_M
            .iter()
            .map(|&r| RadiusLink {
                label: format!("{} km", r / 1000),
                href: link(r, service_id),
                active: r == radius,
            })
            .collect(),
        service_tiles,
        service_id,
        tab,
        tabs,
        services,
        providers,
        products,
        featured_services,
    };
    render_page(&state, &shell, "pages/explore.html", &page)
}
