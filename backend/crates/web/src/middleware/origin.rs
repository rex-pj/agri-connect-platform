//! CSRF defence: a state-changing request whose `Origin` is not one of ours is
//! refused. Together with SameSite cookies this replaces CSRF tokens.
//!
//! A request with no `Origin` header is allowed through: browsers always send
//! one on cross-site POSTs, while server-to-server callers (scripts calling
//! the JSON API) send none and authenticate with a bearer token.

use axum::{
    extract::Request,
    http::{header, Method},
    middleware::Next,
    response::{IntoResponse, Response},
};
use domain::{Error, ErrorCode};

#[derive(Debug, PartialEq, Eq)]
pub enum OriginDecision {
    Allow,
    Refuse,
}

/// Pure so the rule is testable without a router. `allowed` holds exact
/// origins (`scheme://host[:port]`, no trailing slash).
pub fn decide(method: &Method, origin: Option<&str>, allowed: &[String]) -> OriginDecision {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return OriginDecision::Allow;
    }
    match origin {
        None => OriginDecision::Allow,
        Some(o) if allowed.iter().any(|a| a == o) => OriginDecision::Allow,
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

pub async fn enforce(allowed: std::sync::Arc<Vec<String>>, req: Request, next: Next) -> Response {
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    match decide(req.method(), origin, &allowed) {
        OriginDecision::Allow => next.run(req).await,
        OriginDecision::Refuse => Error::new(ErrorCode::CrossOriginRequest)
            .with_detail(format!("origin {origin:?}"))
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed() -> Vec<String> {
        vec!["https://agri.vn".to_owned()]
    }

    #[test]
    fn safe_methods_always_pass() {
        assert_eq!(
            decide(&Method::GET, Some("https://evil.example"), &allowed()),
            OriginDecision::Allow
        );
    }

    #[test]
    fn our_origin_passes_and_others_are_refused() {
        assert_eq!(
            decide(&Method::POST, Some("https://agri.vn"), &allowed()),
            OriginDecision::Allow
        );
        assert_eq!(
            decide(&Method::POST, Some("https://evil.example"), &allowed()),
            OriginDecision::Refuse
        );
        // "null" is what sandboxed frames and file:// pages send.
        assert_eq!(
            decide(&Method::DELETE, Some("null"), &allowed()),
            OriginDecision::Refuse
        );
    }

    #[test]
    fn a_lookalike_origin_is_refused() {
        assert_eq!(
            decide(
                &Method::POST,
                Some("https://agri.vn.evil.example"),
                &allowed()
            ),
            OriginDecision::Refuse
        );
        assert_eq!(
            decide(&Method::POST, Some("http://agri.vn"), &allowed()),
            OriginDecision::Refuse
        );
    }

    #[test]
    fn no_origin_header_passes() {
        assert_eq!(
            decide(&Method::PATCH, None, &allowed()),
            OriginDecision::Allow
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
