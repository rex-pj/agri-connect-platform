//! GET /admin → /admin/dashboard, and the dashboard: the platform counts,
//! then the business ones (leads, steady areas, paying partners, revenue),
//! then the report weeks' leads, plus a warning while demo listings are mixed
//! into the real ones. The weekly leads are the demand report's: one
//! definition of a lead everywhere.

use std::collections::HashMap;

use application::{
    authz::Actor,
    catalogue::{
        PERM_PARTNER_MANAGE, PERM_REPORT_DEMAND_READ, PERM_SETTING_MANAGE, PERM_SYSTEM_DEMO_MANAGE,
    },
    constants::{CALL_STATS_DAYS, NEW_BUSINESS_DAYS},
    features::{Feature, Stage},
    outputs::{AdminStatsDto, DemandAreaDto, DemandReportDto, DemandWeekDto, Figure},
    settings::SettingGroup,
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::Error;
use serde::Serialize;

use crate::{
    app_state::AppState,
    constants::DASHBOARD_NEAR_AREAS,
    locale::Locale,
    middleware::auth::Viewer,
    pages::{
        admin::{demand::area_hrefs, menu_href, render_admin, url},
        listing::with_query,
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
    /// The page behind the figure, when the viewer may open it.
    pub href: Option<String>,
    /// What the figure counts, said under it when the label cannot.
    pub note: Option<String>,
    /// The figure as a whole percentage of a total, drawn as a bar.
    pub share: Option<i64>,
    /// How the figure moved, said at the foot of the card.
    pub change: Option<CardChange>,
}

/// A card's movement: by how much (`delta`, none when it did not move),
/// the words after it, and its way (`up`, `down`, `flat`: the sign, the
/// arrow and the `stat-card__change--<way>` class).
#[derive(Serialize)]
pub struct CardChange {
    pub delta: Option<i64>,
    pub text: String,
    pub way: &'static str,
}

/// Growth of a total: what was added in the last `days` days.
fn added(locale: &Locale, count: i64, days: i64) -> CardChange {
    let (key, way) = if count > 0 {
        ("admin_stats.new_in_days", "up")
    } else {
        ("admin_stats.none_new", "flat")
    };
    CardChange {
        delta: (count > 0).then_some(count),
        text: locale.text_with(key, &[("days", &days)]),
        way,
    }
}

/// A flow against the same window just before it.
fn versus(locale: &Locale, now: i64, before: i64, days: i64) -> CardChange {
    let diff = now - before;
    let (key, way) = match diff.signum() {
        1 => ("admin_stats.versus", "up"),
        -1 => ("admin_stats.versus", "down"),
        _ => ("admin_stats.change_flat", "flat"),
    };
    CardChange {
        delta: (diff != 0).then_some(diff.abs()),
        text: locale.text_with(key, &[("days", &days)]),
        way,
    }
}

/// `part` as a whole percentage of `whole`, rounded; none of nothing.
fn percent(part: i64, whole: i64) -> Option<i64> {
    (whole > 0).then(|| (part * 100 + whole / 2) / whole)
}

/// A titled row of cards, as the admin menu groups its pages.
#[derive(Serialize)]
pub struct StatGroup {
    pub title: String,
    pub cards: Vec<StatCard>,
}

pub(crate) fn card(label: String, value: i64, icon: &'static str, tone: &'static str) -> StatCard {
    StatCard {
        label,
        value,
        money: false,
        icon,
        tone,
        href: None,
        note: None,
        share: None,
        change: None,
    }
}

/// What the platform holds: providers, the verified share of them, supplies
/// on sale, accounts; each card leads to its list.
fn platform_cards(locale: &Locale, actor: &Actor, s: &AdminStatsDto) -> Vec<StatCard> {
    let verified = percent(s.verified_businesses, s.total_businesses);
    vec![
        StatCard {
            href: menu_href(actor, "admin_providers"),
            change: Some(added(locale, s.new_businesses, NEW_BUSINESS_DAYS)),
            ..card(
                locale.text("admin_stats.providers"),
                s.total_businesses,
                "fa-store",
                "green",
            )
        },
        StatCard {
            href: menu_href(actor, "admin_providers"),
            share: verified,
            note: verified
                .map(|p| locale.text_with("admin_stats.verified_share", &[("percent", &p)])),
            ..card(
                locale.text("admin_stats.verified"),
                s.verified_businesses,
                "fa-circle-check",
                "blue",
            )
        },
        StatCard {
            href: menu_href(actor, "admin_products"),
            ..card(
                locale.text("admin_stats.products"),
                s.active_products,
                "fa-box",
                "amber",
            )
        },
        StatCard {
            href: menu_href(actor, "admin_users"),
            ..card(
                locale.text("admin_stats.users"),
                s.total_users,
                "fa-users",
                "violet",
            )
        },
    ]
}

/// The leads of the last `CALL_STATS_DAYS` against the days before: the
/// first card of the business row, the figure partners are sold.
fn calls_card(locale: &Locale, actor: &Actor, s: &AdminStatsDto) -> StatCard {
    StatCard {
        href: menu_href(actor, "admin_demand"),
        change: Some(versus(
            locale,
            s.recent_calls,
            s.previous_calls,
            CALL_STATS_DAYS,
        )),
        ..card(
            locale.text_with("admin_stats.calls", &[("days", &CALL_STATS_DAYS)]),
            s.recent_calls,
            "fa-phone",
            "green",
        )
    }
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

/// The areas (provinces and wards; not "no area") with the most weeks at
/// the bar, then the most leads (a hidden total by what it is at least);
/// only those with a lead. Shown as the report shows them: each ward under
/// its province (listed for it even when not among the nearest), the
/// provinces by their nearest area, the wards of each by their own rank.
fn near_areas(report: &DemandReportDto) -> Vec<DemandAreaDto> {
    let mut ranked: Vec<(usize, &DemandAreaDto)> = report
        .areas
        .iter()
        .enumerate()
        .filter(|(_, a)| a.label.is_some() && a.total.value != Some(0))
        .collect();
    ranked.sort_by_key(|(_, a)| {
        (
            std::cmp::Reverse(a.weeks_at_bar),
            std::cmp::Reverse(a.total.value.or(a.total.at_least).unwrap_or(0)),
        )
    });
    // Each chosen area's place among the nearest, by its index in the report.
    let place: HashMap<usize, usize> = ranked
        .iter()
        .take(DASHBOARD_NEAR_AREAS)
        .enumerate()
        .map(|(p, (i, _))| (*i, p))
        .collect();

    // The report's tree: a top-level row, then the wards nested under it.
    let mut groups: Vec<Vec<(usize, &DemandAreaDto)>> = Vec::new();
    for (i, a) in report.areas.iter().enumerate() {
        match groups.last_mut() {
            Some(group) if a.nested => group.push((i, a)),
            _ => groups.push(vec![(i, a)]),
        }
    }
    let mut picked: Vec<(usize, Vec<&DemandAreaDto>)> = groups
        .into_iter()
        .filter_map(|group| {
            let best = group
                .iter()
                .filter_map(|(i, _)| place.get(i))
                .min()
                .copied()?;
            let mut rows = group.into_iter();
            let (_, top) = rows.next()?;
            let mut wards: Vec<(usize, &DemandAreaDto)> = rows
                .filter_map(|(i, a)| place.get(&i).map(|p| (*p, a)))
                .collect();
            wards.sort_by_key(|(p, _)| *p);
            let rows = std::iter::once(top).chain(wards.into_iter().map(|(_, a)| a));
            Some((best, rows.collect()))
        })
        .collect();
    picked.sort_by_key(|(best, _)| *best);
    picked
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .cloned()
        .collect()
}

#[derive(Serialize)]
pub struct DashboardPage {
    pub groups: Vec<StatGroup>,
    /// The report weeks' leads (with `report.demand.read`), newest first.
    pub weeks: Vec<WeekRow>,
    /// The report's weeks, oldest first, as the charts draw them.
    pub report_weeks: Vec<DemandWeekDto>,
    /// The leads and the searches of those weeks, for the charts.
    pub lead_weeks: Vec<Figure>,
    pub search_weeks: Vec<Figure>,
    /// The areas nearest to steady, and the link to each one's report.
    pub near: Vec<DemandAreaDto>,
    pub near_hrefs: Vec<String>,
    pub steady_weekly_leads: i64,
    pub min_count: i64,
    pub report_href: Option<String>,
    /// Demo businesses still listed; only for those who may remove them.
    pub demo_count: Option<u64>,
    /// The features not open to farmers (in preview or off), and the
    /// settings tab that opens them; only for those who may change them.
    pub features_closed: Option<FeaturesClosed>,
}

#[derive(Serialize)]
pub struct FeaturesClosed {
    pub count: usize,
    pub href: String,
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
        cards: platform_cards(locale, &actor, &stats),
    }];

    let mut business = vec![calls_card(locale, &actor, &stats)];
    let mut weeks = Vec::new();
    let mut report_weeks = Vec::new();
    let mut lead_weeks = Vec::new();
    let mut search_weeks = Vec::new();
    let mut near = Vec::new();
    let mut near_hrefs = Vec::new();
    let mut steady_weekly_leads = 0;
    let mut min_count = 0;
    let mut report_href = None;
    // Areas ready to show partners: every report week at the bar.
    if actor.can(PERM_REPORT_DEMAND_READ) {
        let report = state.usecases.demand.report(&actor, None).await?;
        let href = url("admin_demand", &[])?;
        business.push(StatCard {
            href: Some(href.clone()),
            note: Some(locale.text_with(
                "admin_stats.steady_note",
                &[
                    ("weeks", &report.weeks.len()),
                    ("bar", &report.steady_weekly_leads),
                ],
            )),
            ..card(
                locale.text("admin_stats.steady_areas"),
                report.steady_areas as i64,
                "fa-chart-column",
                "blue",
            )
        });
        weeks = week_rows(&report);
        near = near_areas(&report);
        near_hrefs = area_hrefs(&near)?;
        steady_weekly_leads = report.steady_weekly_leads;
        min_count = report.min_count;
        lead_weeks = report.all.weeks;
        search_weeks = report.searches;
        report_weeks = report.weeks;
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
            note: Some(locale.text("admin_stats.revenue_note")),
            ..card(
                locale.text("partner.monthly_revenue"),
                kpis.monthly_revenue,
                "fa-coins",
                "amber",
            )
        });
    }
    groups.push(StatGroup {
        title: locale.text("admin_menu.business"),
        cards: business,
    });

    // Asking only when allowed: the banner's button leads to that page.
    let demo_count = if actor.can(PERM_SYSTEM_DEMO_MANAGE) {
        let status = state.usecases.setup.demo_status(&actor).await?;
        (status.demo.businesses > 0).then_some(status.demo.businesses)
    } else {
        None
    };
    // So a part left in preview or off is not forgotten.
    let features_closed = if actor.can(PERM_SETTING_MANAGE) {
        let settings = state.usecases.settings.current().await?;
        let count = Feature::ALL
            .iter()
            .filter(|f| settings.stage(**f) != Stage::On)
            .count();
        let href = with_query(
            &url("admin_settings", &[])?,
            &[("tab", SettingGroup::Features.code())],
        );
        (count > 0).then_some(FeaturesClosed { count, href })
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
            report_weeks,
            lead_weeks,
            search_weeks,
            near,
            near_hrefs,
            steady_weekly_leads,
            min_count,
            report_href,
            demo_count,
            features_closed,
        },
    )
}
