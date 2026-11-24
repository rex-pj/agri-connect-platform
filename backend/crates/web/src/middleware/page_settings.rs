//! Reads the platform settings once for each request that may render a
//! page, so every page of it (app, admin, auth, error) is drawn with the
//! same name and help lines without each handler reading them: `current()`
//! answers within the request. Read on every request, never kept between
//! them, so an admin's change shows on the next one.
//!
//! Skipped for the JSON API, assets and uploaded files, which draw no page.
//! A failed read is logged and the request goes on with the defaults: the
//! error page of a database outage still renders, under the deployment's
//! own name.

use std::sync::Arc;

use application::settings::Settings;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use tracing::warn;

use crate::{app_state::AppState, routes};

tokio::task_local! {
    static CURRENT: Arc<Settings>;
}

/// The settings read for the request being served; `None` outside one (a
/// background task, a JSON request, a unit test).
pub fn current() -> Option<Arc<Settings>> {
    CURRENT.try_with(Arc::clone).ok()
}

/// Whether `path` may render a page.
fn draws_a_page(path: &str) -> bool {
    !(routes::is_asset_path(path) || path.starts_with(routes::API_PREFIX))
}

pub async fn scope_request(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if !draws_a_page(req.uri().path()) {
        return next.run(req).await;
    }
    let settings = match state.usecases.settings.current().await {
        Ok(settings) => settings,
        Err(e) => {
            warn!(error = %e, "settings unreadable; the page uses the defaults");
            Settings::default()
        }
    };
    CURRENT.scope(Arc::new(settings), next.run(req)).await
}

#[cfg(test)]
mod tests {
    use super::draws_a_page;

    #[test]
    fn only_a_request_that_may_draw_a_page_reads_the_settings() {
        for page in ["/", "/admin/settings", "/login", "/no-such-page"] {
            assert!(draws_a_page(page), "{page}");
        }
        for other in [
            "/api/explore/services",
            "/static/css/app.css",
            "/files/7b1c/thumb",
            "/health/live",
            "/favicon.ico",
        ] {
            assert!(!draws_a_page(other), "{other}");
        }
    }
}
