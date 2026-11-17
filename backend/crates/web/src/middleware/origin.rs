//! CSRF defence: a state-changing request whose `Origin` is not one of ours is
//! refused. Together with SameSite cookies this replaces CSRF tokens.
//!
//! A request with no `Origin` header is allowed through: browsers always send
//! one on cross-site POSTs, while server-to-server callers (scripts calling
//! the JSON API) send none and authenticate with a bearer token.
//!
//! Two lists. Ours (APP_URL) may send anything. The other sites allowed to
//! call the API (CORS_ORIGINS, e.g. the Zalo Mini App's host, which every
//! Mini App shares) may write only without a session cookie: they sign in
//! with a bearer token or not at all, so a page there can never act as a
//! visitor who is signed in here.

use axum::{
    extract::Request,
    http::{header, Method},
    middleware::Next,
    response::{IntoResponse, Response},
};
use domain::{Error, ErrorCode};

use crate::cookies::request_cookie;

#[derive(Debug, PartialEq, Eq)]
pub enum OriginDecision {
    Allow,
    Refuse,
}

/// The origins whose writes are accepted: exact origins
/// (`scheme://host[:port]`, no trailing slash).
#[derive(Debug, Clone, Default)]
pub struct Origins {
    /// Our own site: any request.
    pub own: Vec<String>,
    /// Other sites that may call the API: only without a session cookie.
    pub api: Vec<String>,
    /// The names of the session cookies (`CookiePolicy`).
    pub session_cookies: [&'static str; 2],
}

/// Pure so the rule is testable without a router. `has_session`: the request
/// carries one of our session cookies.
pub fn decide(
    method: &Method,
    origin: Option<&str>,
    origins: &Origins,
    has_session: bool,
) -> OriginDecision {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return OriginDecision::Allow;
    }
    match origin {
        None => OriginDecision::Allow,
        Some(o) if origins.own.iter().any(|a| a == o) => OriginDecision::Allow,
        Some(o) if !has_session && origins.api.iter().any(|a| a == o) => OriginDecision::Allow,
        Some(_) => OriginDecision::Refuse,
    }
}

/// `scheme://host[:port]` from a configured URL such as APP_URL.
pub fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|a| !a.is_empty())?;
    Some(format!(
        "{}://{}",
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase()
    ))
}

pub async fn enforce(origins: std::sync::Arc<Origins>, req: Request, next: Next) -> Response {
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    let has_session = origins
        .session_cookies
        .iter()
        .any(|name| request_cookie(req.headers(), name).is_some());
    match decide(req.method(), origin, &origins, has_session) {
        OriginDecision::Allow => next.run(req).await,
        OriginDecision::Refuse => Error::new(ErrorCode::CrossOriginRequest)
            .with_detail(format!("origin {origin:?}"))
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI_APP: &str = "https://h5.zdn.vn";

    fn allowed() -> Origins {
        Origins {
            own: vec!["https://agri.vn".to_owned()],
            api: vec![MINI_APP.to_owned()],
            session_cookies: ["auth_token", "refresh_token"],
        }
    }

    /// A request with one of our session cookies.
    fn signed_in(method: &Method, origin: Option<&str>) -> OriginDecision {
        decide(method, origin, &allowed(), true)
    }

    #[test]
    fn safe_methods_always_pass() {
        assert_eq!(
            signed_in(&Method::GET, Some("https://evil.example")),
            OriginDecision::Allow
        );
    }

    #[test]
    fn our_origin_passes_and_others_are_refused() {
        assert_eq!(
            signed_in(&Method::POST, Some("https://agri.vn")),
            OriginDecision::Allow
        );
        assert_eq!(
            signed_in(&Method::POST, Some("https://evil.example")),
            OriginDecision::Refuse
        );
        // "null" is what sandboxed frames and file:// pages send.
        assert_eq!(
            signed_in(&Method::DELETE, Some("null")),
            OriginDecision::Refuse
        );
    }

    #[test]
    fn a_lookalike_origin_is_refused() {
        assert_eq!(
            signed_in(&Method::POST, Some("https://agri.vn.evil.example")),
            OriginDecision::Refuse
        );
        assert_eq!(
            signed_in(&Method::POST, Some("http://agri.vn")),
            OriginDecision::Refuse
        );
    }

    #[test]
    fn no_origin_header_passes() {
        assert_eq!(signed_in(&Method::PATCH, None), OriginDecision::Allow);
    }

    #[test]
    fn an_api_origin_writes_only_without_a_session() {
        assert_eq!(
            decide(&Method::POST, Some(MINI_APP), &allowed(), false),
            OriginDecision::Allow,
            "the Mini App posts a job with no cookie"
        );
        assert_eq!(
            signed_in(&Method::POST, Some(MINI_APP)),
            OriginDecision::Refuse,
            "a page there never acts as someone signed in here"
        );
        assert_eq!(
            decide(
                &Method::POST,
                Some("https://evil.example"),
                &allowed(),
                false
            ),
            OriginDecision::Refuse
        );
    }

    #[test]
    fn origin_is_derived_from_a_url() {
        assert_eq!(
            origin_of("https://Agri.vn/"),
            Some("https://agri.vn".to_owned())
        );
        assert_eq!(
            origin_of("http://localhost:5173/x?y"),
            Some("http://localhost:5173".to_owned())
        );
        assert_eq!(origin_of("not a url"), None);
    }
}
