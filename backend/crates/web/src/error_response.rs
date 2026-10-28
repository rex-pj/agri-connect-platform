//! Turns a `domain::Error` parked in the response extensions into the body
//! clients see: the error page for a browser asking for a page, JSON for
//! everything else. Every router must sit under this layer: a handler error
//! that bypasses it reaches the client as a bare status with an empty body.

use std::collections::BTreeMap;

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use domain::{Error, ErrorKind};
use serde::Serialize;

use crate::{
    app_state::AppState,
    routings::app_routing::REQUEST_ID_HEADER,
    shell::{self, Shell},
    views,
};

#[derive(Serialize)]
struct ErrorPage {
    status: u16,
    message: String,
    request_id: String,
}

/// Paths that are never pages: the API, assets, probes and uploaded files.
/// Their errors stay JSON (or a bare status) whatever the client accepts.
pub fn is_machine_path(path: &str) -> bool {
    path.starts_with("/api/")
        || path.starts_with("/static/")
        || path.starts_with("/health/")
        || path.starts_with("/files/")
        || path == crate::views::FAVICON_PATH
}

/// A browser navigation (not an API call, not an asset) gets an HTML page;
/// a fetch from an island or a script keeps getting JSON.
fn wants_html(req: &Request) -> bool {
    !is_machine_path(req.uri().path())
        && req
            .headers()
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|accept| accept.contains("text/html"))
}

/// Wire shape of every error: `field -> [message]` for the boxes at fault.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub field_errors: BTreeMap<&'static str, Vec<String>>,
}

pub fn error_body(state: &AppState, err: &Error) -> ErrorBody {
    let locale = state.locale();
    ErrorBody {
        code: err.code().as_str(),
        message: locale.error_text(err.code(), err.params()),
        field_errors: err
            .fields()
            .iter()
            .map(|f| (f.field, vec![locale.error_text(f.code, &f.params)]))
            .collect(),
    }
}

pub async fn render_errors(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let query = req.uri().query().map(str::to_owned);
    let html = wants_html(&req);
    // A visitor landing on an error page from a link is offered a language
    // like on any other page.
    let language_offer = html
        .then(|| shell::language_offer(req.headers(), &state.locales))
        .flatten();
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let mut response = next.run(req).await;

    let Some(err) = response.extensions_mut().remove::<Error>() else {
        return response;
    };

    match err.kind() {
        ErrorKind::Internal => {
            tracing::error!(%method, %path, code = err.code().as_str(), detail = err.detail(), "request failed");
        }
        _ => {
            tracing::debug!(%method, %path, code = err.code().as_str(), detail = err.detail(), "request rejected");
        }
    }

    let status = response.status();
    let mut headers = std::mem::take(response.headers_mut());
    // The body is replaced, so length and type from the original must go.
    headers.remove(header::CONTENT_LENGTH);
    headers.remove(header::CONTENT_TYPE);

    let body = error_body(&state, &err);
    let page = ErrorPage {
        status: status.as_u16(),
        message: body.message.clone(),
        request_id,
    };
    let mut rendered = match html.then(|| {
        let mut shell = Shell::without_session(state.locale(), &path, query.as_deref());
        shell.language_offer = language_offer;
        views::render_page_status(&state, &shell, status, "pages/error.html", &page)
    }) {
        Some(Ok(page)) => page,
        Some(Err(render_err)) => {
            // The error page itself failed; JSON still tells the user something.
            tracing::error!(error = %render_err, "error page failed to render");
            (status, Json(body)).into_response()
        }
        None => (status, Json(body)).into_response(),
    };
    carry_headers(&headers, rendered.headers_mut());
    rendered
}

/// Copies every header of `from` that the rendered page did not set itself,
/// with all of its values: a renewed session sends two `Set-Cookie` lines,
/// and keeping only the first would leave the browser a spent refresh token.
fn carry_headers(from: &HeaderMap, to: &mut HeaderMap) {
    for name in from.keys() {
        if to.contains_key(name) {
            continue;
        }
        for value in from.get_all(name) {
            to.append(name.clone(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{header, HeaderMap, HeaderValue};

    use super::{carry_headers, is_machine_path};

    #[test]
    fn every_cookie_survives_the_error_page_and_page_headers_win() {
        let mut from = HeaderMap::new();
        from.append(header::SET_COOKIE, HeaderValue::from_static("auth_token=a"));
        from.append(
            header::SET_COOKIE,
            HeaderValue::from_static("refresh_token=r"),
        );
        from.insert(header::CACHE_CONTROL, HeaderValue::from_static("public"));
        let mut to = HeaderMap::new();
        to.insert(header::CACHE_CONTROL, HeaderValue::from_static("private"));
        carry_headers(&from, &mut to);
        assert_eq!(to.get_all(header::SET_COOKIE).iter().count(), 2);
        assert_eq!(
            to.get(header::CACHE_CONTROL).map(|v| v.as_bytes()),
            Some(&b"private"[..])
        );
    }

    #[test]
    fn api_assets_probes_and_files_are_not_pages() {
        assert!(is_machine_path("/api/explore/products"));
        assert!(is_machine_path("/static/css/app.css"));
        assert!(is_machine_path("/health/ready"));
        assert!(is_machine_path(
            "/files/00000000-0000-0000-0000-000000000000/thumb"
        ));
        assert!(is_machine_path("/favicon.ico"));
        assert!(!is_machine_path("/products/1/gao"));
        assert!(!is_machine_path("/"));
    }
}
