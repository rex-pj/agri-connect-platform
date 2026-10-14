//! Turns a `domain::Error` parked in the response extensions into the body
//! clients see: the error page for a browser asking for a page, JSON for
//! everything else. Every router must sit under this layer: a handler error
//! that bypasses it reaches the client as a bare status with an empty body.

use std::collections::BTreeMap;

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use domain::{Error, ErrorKind};
use serde::Serialize;

use crate::{
    app_state::AppState, legacy::is_rust_only, routings::app_routing::REQUEST_ID_HEADER, views,
};

#[derive(Serialize)]
struct ErrorPage {
    status: u16,
    message: String,
    request_id: String,
}

/// A browser navigation (not an API call, not an asset) gets an HTML page;
/// a fetch from an island or the SvelteKit server keeps getting JSON.
fn wants_html(req: &Request) -> bool {
    !is_rust_only(req.uri().path())
        && req
            .headers()
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|accept| accept.contains("text/html"))
}

/// Wire shape of every error. `field_errors` keeps the `field -> [message]`
/// layout the SvelteKit app parses until its last page is migrated.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub field_errors: BTreeMap<&'static str, Vec<String>>,
}

pub fn error_body(state: &AppState, err: &Error) -> ErrorBody {
    let locale = &state.locale;
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
    let html = wants_html(&req);
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
    let mut rendered =
        match html.then(|| views::render_page_status(&state, status, "pages/error.html", &page)) {
            Some(Ok(page)) => page,
            Some(Err(render_err)) => {
                // The error page itself failed; JSON still tells the user something.
                tracing::error!(error = %render_err, "error page failed to render");
                (status, Json(body)).into_response()
            }
            None => (status, Json(body)).into_response(),
        };
    for (name, value) in headers.iter() {
        rendered
            .headers_mut()
            .entry(name)
            .or_insert_with(|| value.clone());
    }
    rendered
}
