//! Turns a `domain::Error` parked in the response extensions into the body
//! clients see: the error page for a browser asking for a page, JSON for
//! everything else. Every router must sit under this layer: a handler error
//! that bypasses it reaches the client as a bare status with an empty body.

use std::collections::BTreeMap;

use application::authz::Actor;
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{header, HeaderMap, Uri},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use domain::{Error, ErrorKind};
use serde::Serialize;

use crate::{
    app_state::AppState,
    locale::Locale,
    middleware::auth::Viewer,
    pages::admin::render_admin,
    routes,
    routings::app_routing::REQUEST_ID_HEADER,
    shell::{self, Area, Link, Shell},
    views,
};

#[derive(Serialize)]
struct ErrorPage {
    status: u16,
    message: String,
    request_id: String,
    /// Home, or the visitor's own area when the error is inside it.
    back: Link,
}

/// Who asked, for the frame of the error page: the same session the page
/// would have read. Not looked up for a refused flood (429) nor when the
/// server is failing (internal, unavailable), where it would only add load;
/// those pages show
/// signed out, as they did before.
async fn viewer(state: &AppState, kind: ErrorKind, uri: Uri, headers: HeaderMap) -> Option<Actor> {
    if matches!(
        kind,
        ErrorKind::TooManyRequests | ErrorKind::Internal | ErrorKind::Unavailable
    ) {
        return None;
    }
    let (mut parts, ()) = Request::new(()).into_parts();
    parts.uri = uri;
    parts.headers = headers;
    Viewer::from_request_parts(&mut parts, state)
        .await
        .ok()
        .and_then(|Viewer(actor)| actor)
}

/// Where the error page leads back to: the visitor's area when the error
/// is one of its pages, else home.
fn way_back(locale: &Locale, area: Area, actor: Option<&Actor>, path: &str) -> Link {
    if area.covers(path) {
        return shell::home_area(locale, actor);
    }
    Link {
        href: routes::url("home", &[]).unwrap_or_else(|_| "/".to_owned()),
        icon: "fa-house",
        label: locale.text("common.back_home"),
    }
}

/// Paths that are never pages: the API, assets, probes and uploaded files.
/// Their errors stay JSON (or a bare status) whatever the client accepts.
pub fn is_machine_path(path: &str) -> bool {
    path.starts_with(crate::routes::API_PREFIX) || crate::routes::is_asset_path(path)
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
    let uri = req.uri().clone();
    let path = uri.path().to_owned();
    let query = uri.query().map(str::to_owned);
    let html = wants_html(&req);
    // Only a page needs to know who asked; kept until the outcome is known.
    let request_headers = html.then(|| req.headers().clone());
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
    let rendered = match request_headers {
        None => None,
        Some(request_headers) => {
            let actor = viewer(&state, err.kind(), uri, request_headers).await;
            let locale = state.locale();
            let area = Area::of(actor.as_ref());
            let page = ErrorPage {
                status: status.as_u16(),
                message: body.message.clone(),
                request_id,
                back: way_back(locale, area, actor.as_ref(), &path),
            };
            // Inside the admin panel or the partner's report, the error keeps
            // that frame; anywhere else it is an app page, signed in or not.
            Some(match (area.covers(&path), actor.as_ref()) {
                (true, Some(admin)) if area == Area::Admin => render_admin(
                    &state,
                    admin,
                    &path,
                    status,
                    "pages/admin/error.html",
                    &page,
                ),
                _ => {
                    let template = if area == Area::Partner && area.covers(&path) {
                        "pages/partner_error.html"
                    } else {
                        "pages/error.html"
                    };
                    let mut shell = match actor.as_ref() {
                        Some(a) => Shell::new(locale, &path, query.as_deref(), Some(a)),
                        None => Shell::without_session(locale, &path, query.as_deref()),
                    };
                    shell.language_offer = language_offer;
                    // The frame as on any page: no tab to a hidden feature.
                    if let Some(settings) = crate::middleware::page_settings::current() {
                        shell.apply_settings(&settings);
                    }
                    views::render_page_status(&state, &shell, status, template, &page)
                }
            })
        }
    };
    let mut rendered = match rendered {
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
