//! Transitional reverse proxy to the SvelteKit app. Rust is the front door;
//! any page it does not serve yet is forwarded here, so the old app keeps
//! working page by page until its last route is migrated. Deleted in S20.
//!
//! The browser's Host header is forwarded unchanged: SvelteKit compares a
//! form's Origin with the host it believes it serves, so rewriting Host to the
//! Node address would reject every form POST as cross-site.

use axum::{
    body::Body,
    extract::Request,
    http::{header, uri::PathAndQuery, HeaderMap, HeaderName, HeaderValue, Uri},
    response::Response,
};
use domain::{Error, ErrorCode};

use crate::middleware::client_ip::ClientIp;
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};

/// Connection-scoped headers (RFC 9110 §7.6.1): meaningful for one hop only,
/// so forwarding them would corrupt the next connection.
const HOP_BY_HOP: &[HeaderName] = &[
    header::CONNECTION,
    header::PROXY_AUTHENTICATE,
    header::PROXY_AUTHORIZATION,
    header::TE,
    header::TRAILER,
    header::TRANSFER_ENCODING,
    header::UPGRADE,
];

fn strip_hop_by_hop(headers: &mut HeaderMap) {
    for name in HOP_BY_HOP {
        headers.remove(name);
    }
    headers.remove("keep-alive");
}

#[derive(Clone)]
pub struct LegacyProxy {
    client: Client<HttpConnector, Body>,
    scheme: axum::http::uri::Scheme,
    authority: axum::http::uri::Authority,
    https: bool,
}

impl LegacyProxy {
    /// `base` is the SvelteKit server, e.g. `http://legacy:3001`. Only its
    /// scheme and authority are used; the request keeps its own path.
    pub fn new(base: &Uri, https: bool) -> Result<Self, String> {
        let scheme = base
            .scheme()
            .cloned()
            .ok_or("LEGACY_FRONTEND_URL needs a scheme")?;
        let authority = base
            .authority()
            .cloned()
            .ok_or("LEGACY_FRONTEND_URL needs a host")?;
        if scheme != axum::http::uri::Scheme::HTTP {
            return Err("LEGACY_FRONTEND_URL must be http:// (it is an internal hop)".into());
        }
        let client = Client::builder(TokioExecutor::new()).build_http();
        Ok(Self {
            client,
            scheme,
            authority,
            https,
        })
    }

    pub async fn forward(&self, mut req: Request) -> Result<Response, Error> {
        // The resolved client, not the TCP peer (which is Caddy in production).
        let peer = req.extensions().get::<ClientIp>().map(|c| c.0);
        let path = req
            .uri()
            .path_and_query()
            .cloned()
            .unwrap_or_else(|| PathAndQuery::from_static("/"));
        *req.uri_mut() = Uri::builder()
            .scheme(self.scheme.clone())
            .authority(self.authority.clone())
            .path_and_query(path)
            .build()
            .map_err(|e| Error::internal(format!("legacy uri: {e}")))?;

        let headers = req.headers_mut();
        strip_hop_by_hop(headers);
        if let Some(ip) = peer {
            if let Ok(v) = HeaderValue::from_str(&ip.to_string()) {
                headers.insert("x-forwarded-for", v);
            }
        }
        headers.insert(
            "x-forwarded-proto",
            HeaderValue::from_static(if self.https { "https" } else { "http" }),
        );

        let mut response = self.client.request(req).await.map_err(|e| {
            Error::new(ErrorCode::LegacyFrontendUnavailable).with_detail(e.to_string())
        })?;
        strip_hop_by_hop(response.headers_mut());
        Ok(response.map(Body::new))
    }
}

/// Paths Rust owns even when it has no page for them: they must answer with
/// Rust's own coded 404, never a SvelteKit page.
pub fn is_rust_only(path: &str) -> bool {
    path.starts_with("/api/") || path.starts_with("/static/") || path.starts_with("/health/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_static_and_health_are_never_proxied() {
        assert!(is_rust_only("/api/explore/products"));
        assert!(is_rust_only("/static/css/app.css"));
        assert!(is_rust_only("/health/ready"));
        assert!(!is_rust_only("/products/1/gao"));
        assert!(!is_rust_only("/"));
    }

    #[test]
    fn only_plain_http_bases_are_accepted() {
        let ok: Uri = "http://legacy:3001".parse().unwrap_or_default();
        assert!(LegacyProxy::new(&ok, false).is_ok());
        let https: Uri = "https://legacy:3001".parse().unwrap_or_default();
        assert!(LegacyProxy::new(&https, false).is_err());
        let no_host: Uri = "/just/a/path".parse().unwrap_or_default();
        assert!(LegacyProxy::new(&no_host, false).is_err());
    }
}
