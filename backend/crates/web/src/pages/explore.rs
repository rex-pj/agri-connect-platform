//! GET /explore — services, products and providers near the visitor, in three
//! tabs (`tab=providers`, `tab=products`; links, so they work without JS).
//! Each tab's list is paged on its own: `page` belongs to the chosen tab
//! (`listing::tab_page`), and each list's page links carry its tab.
//! Before a position is known it features services, like the home page.
//!
//! The position comes from the `ac-geolocate` island (`lat`, `lng`) or, with
//! or without JS, from a location preset chosen in a GET form
//! (`location_id`). Service and radius filters are links that keep the
//! position. Coordinates are in the query string but never in the logs: the
//! request span records the path only.

use crate::{middleware::client_ip::ClientIp, pages::visitor};
use application::outputs::{CategoryDto, LocationDto, ProductDto};
use application::{
    constants::{LAT_MAX, LAT_MIN, LNG_MAX, LNG_MIN},
    inputs::{GeoSearchParams, SearchInput, SearchPlace},
};
use axum::{extract::State, response::Response, Extension};
use domain::{constants::CATEGORY_TYPE_SERVICE, Error};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::{km, EXPLORE_DEFAULT_RADIUS_M, EXPLORE_RADII_M},
    extract::ApiQuery,
    pages::{
        cards::{rated_providers, rated_services, tile, CategoryTile, ProviderCard, ServiceCard},
        listing::{parse_opt, past_the_end, tab_page, with_query, Pagination},
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
    /// The page of the chosen tab's list (`listing::tab_page`).
    pub page: String,
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
    /// Sponsored services of the area, shown first and labelled.
    pub sponsored: Vec<ServiceCard>,
    /// Services whose provider is within the radius, nearest first: one
    /// page of them. Each list is paged on its own (`listing::tab_page`).
    pub services: Vec<ServiceCard>,
    pub service_total: i64,
    pub service_pages: Option<Pagination>,
    pub providers: Vec<ProviderCard>,
    pub provider_total: i64,
    pub provider_pages: Option<Pagination>,
    /// Products whose seller is within the radius, nearest first.
    pub products: Vec<ProductDto>,
    pub product_total: i64,
    pub product_pages: Option<Pagination>,
    /// Before a position is known: the same featured services as the home page.
    pub featured_services: Vec<ServiceCard>,
}

/// The three lists near a position, each one page of its own.
#[derive(Default)]
struct Found {
    sponsored: Vec<ServiceCard>,
    services: Vec<ServiceCard>,
    service_total: i64,
    service_pages: Option<Pagination>,
    providers: Vec<ProviderCard>,
    provider_total: i64,
    provider_pages: Option<Pagination>,
    products: Vec<ProductDto>,
    product_total: i64,
    product_pages: Option<Pagination>,
}

const TAB_SERVICES: &str = "services";
pub(super) const TAB_PROVIDERS: &str = "providers";
pub(super) const TAB_PRODUCTS: &str = "products";

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
    ip: Option<Extension<ClientIp>>,
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
    // The query of this page with the position kept and the given filters and tab.
    let params_for = |radius: i64, service: Option<i32>, tab: &str| {
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
        params
    };
    let link_to = |radius: i64, service: Option<i32>, tab: &str| {
        let params = params_for(radius, service, tab);
        let borrowed: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        with_query(&path, &borrowed)
    };
    let link = |radius: i64, service: Option<i32>| link_to(radius, service, tab);
    // One tab's own page links: its list paged on its own, the position,
    // filters and tab kept (`tab_page`).
    let pages_of = |t: &str, page: i64, total_pages: i64| {
        let params = params_for(radius, service_id, t);
        let borrowed: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        Pagination::new(&path, &borrowed, page, total_pages)
    };
    let page_no = parse_opt::<i64>(&q.page).unwrap_or(1);

    let mut found = Found::default();
    let featured_services = match &position {
        // No position yet: what the home page features, instead of results.
        None => {
            rated_services(
                &state,
                explore.get_explore_data(None).await?.featured_services,
            )
            .await?
        }
        Some(p) => {
            let (lat, lng) = p.coordinates();
            let params = || GeoSearchParams {
                lat,
                lng,
                radius: radius as f64,
                service_id,
            };
            let (services, providers, products) = tokio::try_join!(
                explore.search_services_page(params(), tab_page(tab, TAB_SERVICES, page_no)),
                explore.search_providers_page(params(), tab_page(tab, TAB_PROVIDERS, page_no)),
                explore.search_products_page(params(), tab_page(tab, TAB_PRODUCTS, page_no)),
            )?;
            let chosen = match tab {
                TAB_PROVIDERS => (providers.page, providers.total_pages),
                TAB_PRODUCTS => (products.page, products.total_pages),
                _ => (services.page, services.total_pages),
            };
            let params = params_for(radius, service_id, tab);
            let borrowed: Vec<(&str, &str)> =
                params.iter().map(|(k, v)| (*k, v.as_str())).collect();
            if let Some(redirect) = past_the_end(&path, &borrowed, chosen.0, chosen.1) {
                return redirect;
            }
            // A near-me search is demand in that area, found or not; paging
            // through its results is not a new search.
            if page_no <= 1 {
                state.usecases.demand.record_search(SearchInput {
                    query: String::new(),
                    category_id: service_id,
                    place: match p {
                        Position::Preset(l) => SearchPlace::Preset(l.id),
                        Position::Device { lat, lng, .. } => SearchPlace::Point {
                            lat: *lat,
                            lng: *lng,
                        },
                    },
                    results: services.total + providers.total + products.total,
                    visitor: visitor(&shell, ip.as_ref().map(|e| &e.0)),
                });
            }
            // The area's sponsored providers come first on the first page,
            // labelled, taken from that page's rows; the rest keep their
            // order, so no service shows twice across the pages.
            let (sponsored, rest) = if services.page == 1 {
                state
                    .usecases
                    .partner
                    .feature_sponsored(Some(p.coordinates()), service_id, services.items)
                    .await?
            } else {
                (Vec::new(), services.items)
            };
            found = Found {
                sponsored: rated_services(&state, sponsored).await?,
                services: rated_services(&state, rest).await?,
                service_total: services.total,
                service_pages: pages_of(TAB_SERVICES, services.page, services.total_pages),
                providers: rated_providers(
                    &state,
                    providers
                        .items
                        .into_iter()
                        .map(ProviderCard::from)
                        .collect(),
                )
                .await?,
                provider_total: providers.total,
                provider_pages: pages_of(TAB_PROVIDERS, providers.page, providers.total_pages),
                products: products.items,
                product_total: products.total,
                product_pages: pages_of(TAB_PRODUCTS, products.page, products.total_pages),
            };
            Vec::new()
        }
    };
    let tabs = [
        // The order of the bottom tabs and the search page's.
        (TAB_SERVICES, "nav.services", found.service_total),
        (TAB_PRODUCTS, "nav.products", found.product_total),
        (TAB_PROVIDERS, "nav.shops", found.provider_total),
    ]
    .into_iter()
    .map(|(t, label, count)| TabLink {
        tab: t,
        label: state.locale().text(label),
        href: link_to(radius, service_id, t),
        count: Some(count),
        active: t == tab,
    })
    .collect();

    // Every sector's: the page has no sector switch, so none narrows it.
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
        radius_km: km(radius),
        wider: EXPLORE_RADII_M
            .iter()
            .find(|&&r| r > radius)
            .map(|&r| RadiusLink {
                label: state.locale().text_with("explore.km", &[("km", &km(r))]),
                href: link(r, service_id),
                active: false,
            }),
        radius_links: EXPLORE_RADII_M
            .iter()
            .map(|&r| RadiusLink {
                label: state.locale().text_with("explore.km", &[("km", &km(r))]),
                href: link(r, service_id),
                active: r == radius,
            })
            .collect(),
        service_tiles,
        service_id,
        tab,
        tabs,
        sponsored: found.sponsored,
        services: found.services,
        service_total: found.service_total,
        service_pages: found.service_pages,
        providers: found.providers,
        provider_total: found.provider_total,
        provider_pages: found.provider_pages,
        products: found.products,
        product_total: found.product_total,
        product_pages: found.product_pages,
        featured_services,
    };
    render_page(&state, &shell, "pages/explore.html", &page)
}
