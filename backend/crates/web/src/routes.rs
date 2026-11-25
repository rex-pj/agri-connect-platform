//! Every navigable URL, by name: templates reach it through the `url_for` Tera function and
//! handlers through `url`, so no page builds a path by concatenation.
//!
//! A pattern change here changes the link everywhere at once; keeping a URL
//! stable is therefore a one-line review, not a grep.

/// Uploaded images. Also the router's patterns, so the served path and the
/// linked one cannot differ.
pub const FILE_PATH: &str = "/files/{id}";
pub const FILE_THUMB_PATH: &str = "/files/{id}/thumb";
const FILES_PREFIX: &str = "/files/";

/// The bundled assets: the router's pattern and the prefix `asset_url`
/// links under.
pub const STATIC_ROUTE: &str = "/static/{*path}";
pub const STATIC_PREFIX: &str = "/static/";

/// The map's tiles: the router's pattern, and the template the map
/// fills (Leaflet writes `{z}/{x}/{y}` the same way). Never a page.
pub const MAP_TILE_PATH: &str = "/map/tiles/{z}/{x}/{y}";
const MAP_TILES_PREFIX: &str = "/map/tiles/";

/// The probes: the router's paths and the `healthcheck` command's.
pub const HEALTH_LIVE_PATH: &str = "/health/live";
pub const HEALTH_READY_PATH: &str = "/health/ready";
const HEALTH_PREFIX: &str = "/health/";

/// The JSON API.
pub const API_PREFIX: &str = "/api/";

/// Requests for an asset, an uploaded file, the favicon or a probe: never a
/// page, never a session.
pub fn is_asset_path(path: &str) -> bool {
    path.starts_with(STATIC_PREFIX)
        || path.starts_with(FILES_PREFIX)
        || path.starts_with(HEALTH_PREFIX)
        || path.starts_with(MAP_TILES_PREFIX)
        || path == crate::views::FAVICON_PATH
}

/// (name, pattern). `{param}` segments are filled from the call's arguments.
pub const ROUTES: &[(&str, &str)] = &[
    ("home", "/"),
    ("login", "/login"),
    ("logout", "/logout"),
    ("register", "/register"),
    ("forgot_password", "/forgot-password"),
    ("reset_password", "/reset-password"),
    ("verify_email", "/verify-email"),
    ("setup", "/setup"),
    ("setup_seed", "/setup/seed"),
    ("account", "/account"),
    ("account_security", "/account#security"),
    ("account_password", "/account/password"),
    ("account_email_verification", "/account/email-verification"),
    ("account_profile", "/account/profile"),
    ("account_email", "/account/email"),
    ("account_email_box", "/account#email"),
    ("account_sessions", "/account/sessions"),
    ("explore", "/explore"),
    ("products", "/supplies"),
    ("services", "/services"),
    ("prices", "/prices"),
    ("job_new", "/jobs/new"),
    ("job_post", "/jobs"),
    ("search", "/search"),
    ("provider_dashboard", "/provider/dashboard"),
    ("provider_profile", "/provider/profile"),
    ("provider_product_new", "/provider/supplies/new"),
    ("provider_product_edit", "/provider/supplies/{id}/edit"),
    ("provider_product_delete", "/provider/supplies/{id}/delete"),
    ("provider_service_new", "/provider/services/new"),
    ("provider_service_edit", "/provider/services/{id}/edit"),
    ("provider_service_delete", "/provider/services/{id}/delete"),
    ("provider_inquiries", "/provider/inquiries"),
    ("provider_inquiry_status", "/provider/inquiries/{id}/status"),
    ("provider_reviews", "/provider/reviews"),
    ("provider_jobs", "/provider/jobs"),
    ("provider_review_reply", "/provider/reviews/{id}/reply"),
    ("product", "/supplies/{id}/{slug}"),
    ("product_category", "/supplies/category/{id}/{slug}"),
    ("service_category", "/services/category/{id}/{slug}"),
    ("service", "/services/{id}/{slug}"),
    ("providers", "/providers"),
    ("call_log", "/api/explore/calls"),
    ("provider", "/providers/{id}/{slug}"),
    ("contact", "/contact"),
    ("review_submit", "/reviews"),
    ("language", "/language"),
    ("sector", "/sector"),
    ("admin", "/admin"),
    ("admin_dashboard", "/admin/dashboard"),
    ("admin_users", "/admin/users"),
    ("admin_user", "/admin/users/{id}"),
    ("admin_user_role", "/admin/users/{id}/role"),
    ("admin_user_status", "/admin/users/{id}/status"),
    ("admin_user_profile", "/admin/users/{id}/profile"),
    (
        "admin_user_password_reset",
        "/admin/users/{id}/password-reset",
    ),
    ("admin_roles", "/admin/roles"),
    ("admin_role", "/admin/roles/{id}"),
    ("admin_role_permissions", "/admin/roles/{id}/permissions"),
    ("admin_categories", "/admin/categories"),
    ("admin_category", "/admin/categories/{id}"),
    ("admin_category_status", "/admin/categories/{id}/status"),
    ("admin_category_slug", "/api/admin/categories/slug"),
    ("admin_icons", "/api/admin/icons"),
    ("place_ward", "/api/places/ward"),
    ("place_wards", "/api/places/wards"),
    ("admin_providers", "/admin/providers"),
    ("admin_provider", "/admin/providers/{id}"),
    (
        "admin_provider_verification",
        "/admin/providers/{id}/verification",
    ),
    ("admin_provider_account", "/admin/providers/{id}/account"),
    (
        "admin_provider_account_delete",
        "/admin/providers/{id}/account/delete",
    ),
    ("admin_provider_status", "/admin/providers/{id}/status"),
    ("admin_inquiries", "/admin/inquiries"),
    ("admin_inquiry_status", "/admin/inquiries/{id}/status"),
    ("admin_reviews", "/admin/reviews"),
    ("admin_demand", "/admin/demand"),
    ("admin_demand_csv", "/admin/demand.csv"),
    ("admin_partners", "/admin/partners"),
    ("admin_partner", "/admin/partners/{id}"),
    ("admin_partner_status", "/admin/partners/{id}/status"),
    ("admin_partner_account", "/admin/partners/{id}/account"),
    ("admin_partner_contracts", "/admin/partners/{id}/contracts"),
    (
        "admin_partner_contract",
        "/admin/partners/{id}/contracts/{cid}",
    ),
    (
        "admin_partner_contract_delete",
        "/admin/partners/{id}/contracts/{cid}/delete",
    ),
    (
        "admin_partner_sponsored",
        "/admin/partners/{id}/contracts/{cid}/sponsored",
    ),
    (
        "admin_partner_sponsored_remove",
        "/admin/partners/{id}/contracts/{cid}/sponsored/{bid}/delete",
    ),
    ("partner_report", "/partner"),
    ("partner_report_csv", "/partner/report.csv"),
    ("admin_review_visibility", "/admin/reviews/{id}/visibility"),
    ("admin_jobs", "/admin/jobs"),
    ("admin_job_status", "/admin/jobs/{id}/status"),
    ("admin_prices", "/admin/prices"),
    ("admin_price_delete", "/admin/prices/{id}/delete"),
    ("admin_alert_kinds", "/admin/alert-kinds"),
    ("admin_alert_kind", "/admin/alert-kinds/{id}"),
    ("admin_alert_kind_status", "/admin/alert-kinds/{id}/status"),
    ("admin_sectors", "/admin/sectors"),
    ("admin_sector", "/admin/sectors/{id}"),
    ("admin_sector_status", "/admin/sectors/{id}/status"),
    ("admin_commodities", "/admin/commodities"),
    ("admin_commodity", "/admin/commodities/{id}"),
    ("admin_commodity_status", "/admin/commodities/{id}/status"),
    ("admin_crops", "/admin/crops"),
    ("admin_crop", "/admin/crops/{id}"),
    ("admin_crop_status", "/admin/crops/{id}/status"),
    ("admin_alerts", "/admin/alerts"),
    ("admin_alert", "/admin/alerts/{id}"),
    ("admin_alert_status", "/admin/alerts/{id}/status"),
    ("admin_locations", "/admin/locations"),
    ("admin_locations_bulk", "/admin/locations/bulk"),
    ("admin_location", "/admin/locations/{id}"),
    ("admin_location_status", "/admin/locations/{id}/status"),
    ("admin_location_delete", "/admin/locations/{id}/delete"),
    ("admin_products", "/admin/supplies"),
    ("admin_product", "/admin/supplies/{id}"),
    ("admin_product_status", "/admin/supplies/{id}/status"),
    ("admin_product_delete", "/admin/supplies/{id}/delete"),
    (
        "admin_product_image_delete",
        "/admin/supplies/{id}/image/delete",
    ),
    ("admin_services", "/admin/services"),
    ("admin_service", "/admin/services/{id}"),
    ("admin_service_status", "/admin/services/{id}/status"),
    ("admin_service_delete", "/admin/services/{id}/delete"),
    (
        "admin_service_image_delete",
        "/admin/services/{id}/image/delete",
    ),
    ("admin_settings", "/admin/settings"),
    ("admin_settings_brand", "/admin/settings/brand"),
    (
        "admin_settings_image_delete",
        "/admin/settings/image/delete",
    ),
    ("admin_logout", "/admin/logout"),
    ("file", FILE_PATH),
    ("file_thumb", FILE_THUMB_PATH),
];

#[derive(Debug, PartialEq, Eq)]
pub enum UrlError {
    UnknownRoute(String),
    MissingParam { route: String, param: String },
    UnusedParam { route: String, param: String },
}

impl std::fmt::Display for UrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownRoute(name) => write!(f, "unknown route {name:?}"),
            Self::MissingParam { route, param } => write!(f, "route {route:?} needs {param:?}"),
            Self::UnusedParam { route, param } => write!(f, "route {route:?} has no {param:?}"),
        }
    }
}

/// Percent-encodes one path segment. Ids are digits and slugs ASCII, but a
/// value with `/` or `?` must not silently change which page a link opens.
fn encode_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The URL for `name` with `params` substituted. Every param must be used and
/// every placeholder filled: a typo is an error, never a broken link.
pub fn url(name: &str, params: &[(&str, &str)]) -> Result<String, UrlError> {
    let pattern = ROUTES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, p)| *p)
        .ok_or_else(|| UrlError::UnknownRoute(name.to_owned()))?;

    let mut out = pattern.to_owned();
    for (key, value) in params {
        let placeholder = format!("{{{key}}}");
        if !out.contains(&placeholder) {
            return Err(UrlError::UnusedParam {
                route: name.to_owned(),
                param: (*key).to_owned(),
            });
        }
        out = out.replace(&placeholder, &encode_segment(value));
    }
    if let Some(start) = out.find('{') {
        let param = out
            .get(start + 1..)
            .and_then(|rest| rest.split('}').next())
            .unwrap_or_default()
            .to_owned();
        return Err(UrlError::MissingParam {
            route: name.to_owned(),
            param,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_asset_prefixes_are_the_routes_own() {
        assert!(STATIC_ROUTE.starts_with(STATIC_PREFIX));
        assert!(FILE_PATH.starts_with(FILES_PREFIX));
        assert!(HEALTH_LIVE_PATH.starts_with(HEALTH_PREFIX));
        assert!(HEALTH_READY_PATH.starts_with(HEALTH_PREFIX));
        assert!(MAP_TILE_PATH.starts_with(MAP_TILES_PREFIX));
        assert!(is_asset_path(crate::views::FAVICON_PATH));
        assert!(!is_asset_path("/admin/dashboard"));
    }

    #[test]
    fn static_and_parameterised_routes() {
        assert_eq!(url("home", &[]), Ok("/".to_owned()));
        assert_eq!(
            url("product", &[("id", "12"), ("slug", "gao-st25")]),
            Ok("/supplies/12/gao-st25".to_owned())
        );
    }

    #[test]
    fn mistakes_are_errors_not_broken_links() {
        assert!(matches!(
            url("prodcut", &[]),
            Err(UrlError::UnknownRoute(_))
        ));
        assert!(matches!(
            url("product", &[("id", "1")]),
            Err(UrlError::MissingParam { .. })
        ));
        assert!(matches!(
            url("home", &[("id", "1")]),
            Err(UrlError::UnusedParam { .. })
        ));
    }

    #[test]
    fn values_cannot_escape_their_segment() {
        assert_eq!(
            url("admin_user", &[("id", "1/../2?x")]),
            Ok("/admin/users/1%2F..%2F2%3Fx".to_owned())
        );
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<_> = ROUTES.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate route name");
    }
}
