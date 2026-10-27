//! GET /admin → /admin/dashboard, and the dashboard: platform counts, plus a
//! warning while demo listings are mixed into the real ones (§8.4).

use application::{
    catalogue::PERM_SYSTEM_DEMO_MANAGE, constants::CALL_STATS_DAYS, outputs::AdminStatsDto,
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::Error;
use serde::Serialize;

use crate::{
    app_state::AppState,
    locale::Locale,
    middleware::auth::Viewer,
    pages::{
        admin::{render_admin, url},
        see_other,
    },
};

#[derive(Serialize)]
pub struct StatCard {
    pub label: String,
    pub value: i64,
    pub icon: &'static str,
    /// Colour modifier of the icon tile (`stat-card__icon--<tone>`).
    pub tone: &'static str,
}

fn cards(locale: &Locale, s: &AdminStatsDto) -> Vec<StatCard> {
    let card = |label: String, value, icon, tone| StatCard {
        label,
        value,
        icon,
        tone,
    };
    vec![
        card(
            locale.text("admin_stats.providers"),
            s.total_businesses,
            "fa-store",
            "green",
        ),
        card(
            locale.text("admin_stats.verified"),
            s.verified_businesses,
            "fa-circle-check",
            "blue",
        ),
        card(
            locale.text("admin_stats.products"),
            s.active_products,
            "fa-box",
            "amber",
        ),
        card(
            locale.text("admin_stats.users"),
            s.total_users,
            "fa-users",
            "violet",
        ),
        card(
            locale.text_with("admin_stats.calls", &[("days", &CALL_STATS_DAYS)]),
            s.recent_calls,
            "fa-phone",
            "green",
        ),
    ]
}

#[derive(Serialize)]
pub struct DashboardPage {
    pub stats: Vec<StatCard>,
    /// Demo businesses still listed; only for those who may remove them.
    pub demo_count: Option<i64>,
}

/// GET /admin
pub async fn index(Viewer(viewer): Viewer) -> Result<Response, Error> {
    let target = if viewer.is_some() {
        "admin_dashboard"
    } else {
        "login"
    };
    see_other(&url(target, &[])?)
}

/// GET /admin/dashboard
pub async fn show(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let stats = state.usecases.stats.get_stats(&actor).await?;
    // Asking only when allowed: the banner's button leads to that page.
    let demo_count = if actor.can(PERM_SYSTEM_DEMO_MANAGE) {
        let status = state.usecases.setup.demo_status(&actor).await?;
        (status.demo_business_count > 0).then_some(status.demo_business_count)
    } else {
        None
    };
    render_admin(
        &state,
        &actor,
        &url("admin_dashboard", &[])?,
        StatusCode::OK,
        "pages/admin/dashboard.html",
        &DashboardPage {
            stats: cards(state.locale(), &stats),
            demo_count,
        },
    )
}
