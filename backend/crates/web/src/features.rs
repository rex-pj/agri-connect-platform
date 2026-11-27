//! The addresses of each feature (`application::features`) and the gate in
//! front of them: one place says which pages and JSON routes a feature
//! owns, so a page cannot be reached while its feature is off, however it
//! is linked.
//!
//! Off, an address answers like one that does not exist (`route_not_found`,
//! never kept by a shared cache, so turning it on shows at once). In preview it answers only
//! an account with `feature.preview`, with `noindex` and never cached;
//! anyone else gets the same 404. The admin's pages of a feature are never
//! gated: its data is prepared before it opens.
//!
//! The ways in (tabs, cards, links) are hidden by whoever draws them, from
//! `FeaturesShown` (`Shell::features`, `Shell::shows`).

use std::sync::Arc;

use application::{
    catalogue::PERM_FEATURE_PREVIEW,
    features::{Feature, Stage},
    settings::Settings,
};
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{header, HeaderValue},
    middleware::Next,
    response::{IntoResponse, Response},
};
use domain::{Error, ErrorCode};
use tracing::warn;

use crate::{app_state::AppState, middleware::auth::Viewer, middleware::page_settings};

/// One rule of a feature's addresses.
#[derive(Debug, Clone, Copy)]
enum Rule {
    /// This path only; a `{…}` segment matches any one segment.
    Exact(&'static str),
    /// This path and every path below it.
    Under(&'static str),
}

use Rule::{Exact, Under};

/// Every feature's addresses: its pages, the former addresses that redirect
/// to them, its JSON routes, the provider's own pages of it.
const RULES: &[(Feature, &[Rule])] = &[
    (
        Feature::Prices,
        &[Under("/prices"), Under("/api/explore/prices")],
    ),
    (Feature::NearMe, &[Under("/explore")]),
    (
        Feature::Supplies,
        &[
            Under("/supplies"),
            Under("/products"),
            Under("/provider/supplies"),
            Under("/provider/products"),
            Under("/api/explore/products"),
            Under("/api/provider/products"),
        ],
    ),
    (Feature::Directory, &[Exact("/providers")]),
    (
        Feature::Reviews,
        &[
            Exact("/reviews"),
            Exact("/api/explore/reviews"),
            Exact("/api/explore/providers/{id}/reviews"),
            Under("/provider/reviews"),
            Under("/api/provider/reviews"),
        ],
    ),
    (
        Feature::Jobs,
        &[
            Under("/jobs"),
            Under("/provider/jobs"),
            Under("/api/explore/jobs"),
            Under("/api/provider/jobs"),
        ],
    ),
    (
        Feature::Articles,
        &[Under("/articles"), Under("/api/explore/articles")],
    ),
];

/// The page that shows `feature` best, by route name: where its preview is
/// tried. None for the reviews, which live on the provider pages.
pub const fn page_route(feature: Feature) -> Option<&'static str> {
    match feature {
        Feature::Prices => Some("prices"),
        Feature::NearMe => Some("explore"),
        Feature::Supplies => Some("products"),
        Feature::Directory => Some("providers"),
        Feature::Reviews => None,
        Feature::Jobs => Some("job_new"),
        Feature::Articles => Some("articles"),
    }
}

/// The JSON lists that serve two features by their query: with a position
/// they are near me (the Zalo Mini App's), without one the provider list is
/// the directory.
const JSON_PROVIDERS: &str = "/api/explore/providers";
const JSON_SERVICES: &str = "/api/explore/services";
const POSITION_PARAM: &str = "lat";

fn segments(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter(|s| !s.is_empty())
}

fn matches(rule: Rule, path: &str) -> bool {
    match rule {
        Exact(pattern) => {
            let (mut want, mut got) = (segments(pattern), segments(path));
            loop {
                match (want.next(), got.next()) {
                    (None, None) => return true,
                    (Some(w), Some(g)) if w == g || w.starts_with('{') => {}
                    _ => return false,
                }
            }
        }
        Under(prefix) => {
            let mut got = segments(path);
            segments(prefix).all(|w| got.next() == Some(w))
        }
    }
}

fn has_position(query: Option<&str>) -> bool {
    query.is_some_and(|q| {
        q.split('&')
            .any(|pair| pair.split('=').next() == Some(POSITION_PARAM))
    })
}

/// The feature that owns `path` (with its `query`), if any.
pub fn feature_of(path: &str, query: Option<&str>) -> Option<Feature> {
    if path == JSON_PROVIDERS || path == JSON_SERVICES {
        return match (has_position(query), path == JSON_PROVIDERS) {
            (true, _) => Some(Feature::NearMe),
            (false, true) => Some(Feature::Directory),
            (false, false) => None,
        };
    }
    RULES
        .iter()
        .find(|(_, rules)| rules.iter().any(|r| matches(*r, path)))
        .map(|(feature, _)| *feature)
}

/// Never kept by a shared cache (Cloudflare) and asked again by the browser
/// each time, as every page (`views::page_headers`): a feature turned on or
/// off shows on the next request, JSON included.
const NOT_STORED: HeaderValue = HeaderValue::from_static("private, no-cache");
const NOINDEX: HeaderValue = HeaderValue::from_static("noindex");
const ROBOTS_HEADER: &str = "x-robots-tag";

/// As if the address did not exist.
fn hidden() -> Response {
    let mut response = Error::new(ErrorCode::RouteNotFound).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, NOT_STORED);
    response
}

/// The settings this request read already (`page_settings`), else read now
/// (a JSON route); unreadable, the defaults (every feature on), as the
/// pages do.
async fn settings(state: &AppState) -> Arc<Settings> {
    if let Some(settings) = page_settings::current() {
        return settings;
    }
    match state.usecases.settings.current().await {
        Ok(settings) => Arc::new(settings),
        Err(e) => {
            warn!(error = %e, "settings unreadable; every feature taken as on");
            Arc::new(Settings::default())
        }
    }
}

/// The gate: in front of every route, inside the error renderer (its 404 is
/// a page or JSON like any other) and after the session is renewed (a
/// previewing admin with an expired access token is still known).
pub async fn gate(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let Some(feature) = feature_of(req.uri().path(), req.uri().query()) else {
        return next.run(req).await;
    };
    match settings(&state).await.stage(feature) {
        Stage::On => next.run(req).await,
        Stage::Off => hidden(),
        Stage::Preview => {
            let (mut parts, body) = req.into_parts();
            let may_preview = match Viewer::from_request_parts(&mut parts, &state).await {
                Ok(Viewer(actor)) => actor.is_some_and(|a| a.can(PERM_FEATURE_PREVIEW)),
                Err(e) => return e.into_response(),
            };
            if !may_preview {
                return hidden();
            }
            let mut response = next.run(Request::from_parts(parts, body)).await;
            let headers = response.headers_mut();
            headers.insert(header::CACHE_CONTROL, NOT_STORED);
            headers.insert(ROBOTS_HEADER, NOINDEX);
            response
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::ROUTES;

    /// Named addresses no feature owns: what the platform is without any
    /// of them (home, services, search, a provider's, service's page, the
    /// contact form, accounts and the provider's own area). The admin's
    /// pages are never gated.
    const ALWAYS_OPEN: &[&str] = &[
        "home",
        "login",
        "logout",
        "register",
        "forgot_password",
        "reset_password",
        "verify_email",
        "setup",
        "setup_seed",
        "account",
        "account_security",
        "account_password",
        "account_email_verification",
        "account_profile",
        "account_email",
        "account_email_box",
        "account_sessions",
        "services",
        "search",
        "provider_dashboard",
        "provider_profile",
        "provider_service_new",
        "provider_service_edit",
        "provider_service_delete",
        "provider_inquiries",
        "provider_inquiry_status",
        "service_category",
        "service",
        "call_log",
        "provider",
        "contact",
        "language",
        "sector",
        "partner_report",
        "partner_report_csv",
        "file",
        "file_thumb",
        // The location box's ward lookup: the provider's profile and the admin's.
        "place_ward",
        "place_wards",
        // Each information page is on, in preview or off by its own row.
        "info_pages",
        "info_page",
    ];

    fn sample(pattern: &str) -> String {
        let path = pattern.split('#').next().unwrap_or(pattern);
        segments(path)
            .map(|s| if s.starts_with('{') { "7" } else { s })
            .fold(String::new(), |acc, s| acc + "/" + s)
            .replace("//", "/")
    }

    #[test]
    fn every_public_address_is_owned_by_a_feature_or_said_to_be_always_open() {
        for (name, pattern) in ROUTES {
            if name.starts_with("admin") {
                assert_eq!(
                    feature_of(&sample(pattern), None),
                    None,
                    "{name}: the admin's pages are never gated"
                );
                continue;
            }
            let owned = feature_of(&sample(pattern), None).is_some();
            let open = ALWAYS_OPEN.contains(name);
            assert!(
                owned != open,
                "{name} ({pattern}): give it to a feature (RULES) or list it in ALWAYS_OPEN, not both"
            );
        }
    }

    #[test]
    fn each_feature_owns_its_own_addresses() {
        let cases = [
            ("/prices", None, Some(Feature::Prices)),
            ("/api/explore/prices/latest", None, Some(Feature::Prices)),
            ("/explore", Some("tab=products"), Some(Feature::NearMe)),
            ("/supplies/3/npk", None, Some(Feature::Supplies)),
            ("/products", None, Some(Feature::Supplies)),
            ("/provider/supplies/3/edit", None, Some(Feature::Supplies)),
            ("/providers", None, Some(Feature::Directory)),
            ("/providers/4/tho-cay", None, None),
            ("/reviews", None, Some(Feature::Reviews)),
            (
                "/api/explore/providers/4/reviews",
                None,
                Some(Feature::Reviews),
            ),
            ("/api/explore/providers/4", None, None),
            ("/jobs/new", None, Some(Feature::Jobs)),
            ("/provider/jobs", Some("page=2"), Some(Feature::Jobs)),
            (
                "/api/explore/providers",
                Some("lat=10.3&lng=106"),
                Some(Feature::NearMe),
            ),
            (
                "/api/explore/providers",
                Some("page=2"),
                Some(Feature::Directory),
            ),
            (
                "/api/explore/services",
                Some("lat=10.3&lng=106"),
                Some(Feature::NearMe),
            ),
            ("/api/explore/services", Some("name=lua"), None),
            ("/services", None, None),
            ("/admin/prices", None, None),
            ("/pricesx", None, None),
            ("/", None, None),
        ];
        for (path, query, want) in cases {
            assert_eq!(feature_of(path, query), want, "{path}?{query:?}");
        }
    }
}
