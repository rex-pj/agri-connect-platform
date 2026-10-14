//! Turns a `domain::Error` parked in the response extensions into the JSON
//! body clients see. Every router must sit under this layer: a handler error
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

use crate::app_state::AppState;

/// Wire shape of every error. `field_errors` keeps the `field -> [message]`
/// layout the SvelteKit app parses until its last page is migrated.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code:    &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub field_errors: BTreeMap<&'static str, Vec<String>>,
}

pub fn error_body(state: &AppState, err: &Error) -> ErrorBody {
    let locale = &state.locale;
    ErrorBody {
        code:    err.code().as_str(),
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

    let mut rendered = (status, Json(error_body(&state, &err))).into_response();
    for (name, value) in headers.iter() {
        rendered.headers_mut().entry(name).or_insert_with(|| value.clone());
    }
    rendered
}
