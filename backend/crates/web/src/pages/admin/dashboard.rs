//! GET /admin → /admin/dashboard, and the dashboard: platform counts, the
//! leads of the last weeks (calls, Zalo chats, inquiries), plus a
//! warning while demo listings are mixed into the real ones.

use application::{
    catalogue::{PERM_PARTNER_MANAGE, PERM_REPORT_DEMAND_READ, PERM_SYSTEM_DEMO_MANAGE},
    constants::CALL_STATS_DAYS,
    outputs::{AdminStatsDto, WeekLeadsDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use chrono::Duration;
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
    views::format_vn_day,
};

#[derive(Serialize)]
pub struct StatCard {
    pub label: String,
    pub value: i64,
    pub icon: &'static str,
    /// Colour modifier of the icon tile (`stat-card__icon--<tone>`).
    pub tone: &'static str,
    /// The page behind the figure, when it has one.
    pub href: Option<String>,
}

fn cards(locale: &Locale, s: &AdminStatsDto) -> Vec<StatCard> {
    let card = |label: String, value, icon, tone| StatCard {
        href: None,
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

/// One week of leads, worded for the table: "01/11 – 07/11".
#[derive(Serialize)]
pub struct WeekRow {
    pub label: String,
    pub calls: i64,
    pub zalo: i64,
    pub inquiries: i64,
    pub total: i64,
}

fn week_rows(weeks: &[WeekLeadsDto]) -> Vec<WeekRow> {
    weeks
        .iter()
        .map(|w| WeekRow {
            // `to` is the first instant after the week: its last day is before it.
            label: format!(
                "{} – {}",
                format_vn_day(w.from),
                format_vn_day(w.to - Duration::seconds(1))
            ),
            calls: w.calls,
            zalo: w.zalo,
            inquiries: w.inquiries,
            total: w.total(),
        })
        .collect()
}

#[derive(Serialize)]
pub struct DashboardPage {
    pub stats: Vec<StatCard>,
    /// The last `LEAD_WEEKS` weeks of leads, newest first.
    pub weeks: Vec<WeekRow>,
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
    let mut stats_cards = cards(state.locale(), &stats);
    // Areas ready to show partners: every report week at the bar.
    if actor.can(PERM_REPORT_DEMAND_READ) {
        let report = state.usecases.demand.report(&actor, None).await?;
        stats_cards.push(StatCard {
            label: state.locale().text_with(
                "admin_stats.steady_areas",
                &[("weeks", &report.weeks.len())],
            ),
            value: report.steady_areas as i64,
            icon: "fa-chart-column",
            tone: "blue",
            href: Some(url("admin_demand", &[])?),
        });
    }
    if actor.can(PERM_PARTNER_MANAGE) {
        let kpis = state.usecases.partner.kpis(&actor).await?;
        let partners_href = url("admin_partners", &[])?;
        stats_cards.push(StatCard {
            label: state.locale().text("partner.paying"),
            value: kpis.paying_partners as i64,
            icon: "fa-handshake",
            tone: "green",
            href: Some(partners_href.clone()),
        });
        stats_cards.push(StatCard {
            label: state.locale().text("admin_stats.monthly_revenue"),
            value: kpis.monthly_revenue,
            icon: "fa-coins",
            tone: "amber",
            href: Some(partners_href),
        });
    }
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
            stats: stats_cards,
            weeks: week_rows(&stats.weeks),
            demo_count,
        },
    )
}
