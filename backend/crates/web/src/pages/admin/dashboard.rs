//! GET /admin → /admin/dashboard, and the dashboard: the platform counts,
//! then the business ones (steady areas, paying partners, revenue) and the
//! leads of the report weeks, plus a warning while demo listings are mixed
//! into the real ones. The weekly leads are the demand report's: one
//! definition of a lead everywhere.

use application::{
    catalogue::{PERM_PARTNER_MANAGE, PERM_REPORT_DEMAND_READ, PERM_SYSTEM_DEMO_MANAGE},
    constants::CALL_STATS_DAYS,
    outputs::{AdminStatsDto, DemandReportDto, Figure},
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
    /// An amount of đồng: shown with the `vnd` filter.
    pub money: bool,
    pub icon: &'static str,
    /// Colour modifier of the icon tile (`stat-card__icon--<tone>`).
    pub tone: &'static str,
    /// The page behind the figure, when it has one.
    pub href: Option<String>,
}

/// A titled row of cards, as the admin menu groups its pages.
#[derive(Serialize)]
pub struct StatGroup {
    pub title: String,
    pub cards: Vec<StatCard>,
}

fn card(label: String, value: i64, icon: &'static str, tone: &'static str) -> StatCard {
    StatCard {
        label,
        value,
        money: false,
        icon,
        tone,
        href: None,
    }
}

fn platform_cards(locale: &Locale, s: &AdminStatsDto) -> Vec<StatCard> {
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

/// One report week of the dashboard's table: "02/11 – 08/11", its leads and
/// its searches, hidden like the report hides them.
#[derive(Serialize)]
pub struct WeekRow {
    pub starts_on: chrono::NaiveDate,
    pub ends_on: chrono::NaiveDate,
    pub leads: Figure,
    pub searches: Figure,
}

/// The report's weeks, newest first.
fn week_rows(report: &DemandReportDto) -> Vec<WeekRow> {
    report
        .weeks
        .iter()
        .zip(&report.all.weeks)
        .zip(&report.searches)
        .rev()
        .map(|((w, leads), searches)| WeekRow {
            starts_on: w.starts_on,
            ends_on: w.ends_on,
            leads: *leads,
            searches: *searches,
        })
        .collect()
}

#[derive(Serialize)]
pub struct DashboardPage {
    pub groups: Vec<StatGroup>,
    /// The report weeks' leads (with `report.demand.read`), newest first.
    pub weeks: Vec<WeekRow>,
    pub min_count: i64,
    pub report_href: Option<String>,
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
    let locale = state.locale();
    let stats = state.usecases.stats.get_stats(&actor).await?;
    let mut groups = vec![StatGroup {
        title: locale.text("admin_stats.platform"),
        cards: platform_cards(locale, &stats),
    }];

    let mut business = Vec::new();
    let mut weeks = Vec::new();
    let mut min_count = 0;
    let mut report_href = None;
    // Areas ready to show partners: every report week at the bar.
    if actor.can(PERM_REPORT_DEMAND_READ) {
        let report = state.usecases.demand.report(&actor, None).await?;
        let href = url("admin_demand", &[])?;
        business.push(StatCard {
            href: Some(href.clone()),
            ..card(
                locale.text_with(
                    "admin_stats.steady_areas",
                    &[("weeks", &report.weeks.len())],
                ),
                report.steady_areas as i64,
                "fa-chart-column",
                "blue",
            )
        });
        weeks = week_rows(&report);
        min_count = report.min_count;
        report_href = Some(href);
    }
    if actor.can(PERM_PARTNER_MANAGE) {
        let kpis = state.usecases.partner.kpis(&actor).await?;
        let href = url("admin_partners", &[])?;
        business.push(StatCard {
            href: Some(href.clone()),
            ..card(
                locale.text("partner.paying"),
                kpis.paying_partners as i64,
                "fa-handshake",
                "green",
            )
        });
        business.push(StatCard {
            href: Some(href),
            money: true,
            ..card(
                locale.text("partner.monthly_revenue"),
                kpis.monthly_revenue,
                "fa-coins",
                "amber",
            )
        });
    }
    if !business.is_empty() {
        groups.push(StatGroup {
            title: locale.text("admin_menu.business"),
            cards: business,
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
            groups,
            weeks,
            min_count,
            report_href,
            demo_count,
        },
    )
}
