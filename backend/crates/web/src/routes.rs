//! Every navigable URL, by name. The Rust counterpart of the SvelteKit
//! `lib/routes.ts`: templates reach it through the `url_for` Tera function and
//! handlers through `url`, so no page builds a path by concatenation.
//!
//! A pattern change here changes the link everywhere at once; keeping a URL
//! stable is therefore a one-line review, not a grep.

/// (name, pattern). `{param}` segments are filled from the call's arguments.
/// Uploaded images. Also the router's patterns, so the served path and the
/// linked one cannot differ.
pub const FILE_PATH: &str = "/files/{id}";
pub const FILE_THUMB_PATH: &str = "/files/{id}/thumb";

pub const ROUTES: &[(&str, &str)] = &[
    ("home", "/"),
    ("login", "/login"),
    ("logout", "/logout"),
    ("register", "/register"),
    ("forgot_password", "/forgot-password"),
    ("reset_password", "/reset-password"),
    ("setup", "/setup"),
    ("setup_seed", "/setup/seed"),
    ("account", "/account"),
    ("account_security", "/account#security"),
    ("account_password", "/account/password"),
    ("explore", "/explore"),
    ("products", "/products"),
    ("services", "/services"),
    ("search", "/search"),
    ("provider_dashboard", "/provider/dashboard"),
    ("product", "/products/{id}/{slug}"),
    ("product_by_id", "/products/{id}"),
    ("product_category", "/products/category/{id}/{slug}"),
    ("service_category", "/services/category/{id}/{slug}"),
    ("provider", "/providers/{id}/{slug}"),
    ("provider_by_id", "/providers/{id}"),
    ("contact", "/contact"),
    ("admin", "/admin"),
    ("admin_dashboard", "/admin/dashboard"),
    ("admin_users", "/admin/users"),
    ("admin_user", "/admin/users/{id}"),
    ("admin_user_role", "/admin/users/{id}/role"),
    ("admin_user_status", "/admin/users/{id}/status"),
    ("admin_roles", "/admin/roles"),
    ("admin_role", "/admin/roles/{id}"),
    ("admin_role_permissions", "/admin/roles/{id}/permissions"),
    ("admin_categories", "/admin/categories"),
    ("admin_category", "/admin/categories/{id}"),
    ("admin_category_status", "/admin/categories/{id}/status"),
    ("admin_explore", "/admin/explore"),
    ("admin_provider", "/admin/explore/{id}"),
    (
        "admin_provider_verification",
        "/admin/explore/{id}/verification",
    ),
    ("admin_provider_account", "/admin/explore/{id}/account"),
    ("admin_inquiries", "/admin/inquiries"),
    ("admin_reviews", "/admin/reviews"),
    ("admin_locations", "/admin/locations"),
    ("admin_location", "/admin/locations/{id}"),
    ("admin_location_status", "/admin/locations/{id}/status"),
    ("admin_location_delete", "/admin/locations/{id}/delete"),
    ("admin_products", "/admin/products"),
    ("admin_services", "/admin/services"),
    ("admin_permissions", "/admin/permissions"),
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
    fn static_and_parameterised_routes() {
        assert_eq!(url("home", &[]), Ok("/".to_owned()));
        assert_eq!(
            url("product", &[("id", "12"), ("slug", "gao-st25")]),
            Ok("/products/12/gao-st25".to_owned())
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
