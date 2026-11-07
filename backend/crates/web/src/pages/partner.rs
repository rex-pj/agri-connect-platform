//! /partner and /partner/report.csv: a partner's own area, the demand report
//! of the areas and categories it pays for (nothing else), with its
//! contracts in force. The area filter and the CSV work as on the admin
//! report.

use application::outputs::PartnerReportDto;
use axum::{extract::State, response::Response};
use domain::Error;
use serde::Serialize;

use crate::{
    app_state::AppState,
    extract::ApiQuery,
    middleware::auth::Viewer,
    pages::{
        admin::demand::{area_options, csv_response, focus_value, AreaOption},
        listing::with_query,
        see_other,
    },
    requests::DemandQuery,
    routes,
    shell::Shell,
    views::render_page,
};

/// The file name the partner's CSV downloads as.
pub const PARTNER_CSV_NAME: &str = "partner-report.csv";

fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("partner links: {e}")))
}

#[derive(Serialize)]
pub struct PartnerPage {
    pub own: PartnerReportDto,
    pub area: String,
    pub areas: Vec<AreaOption>,
    pub action: String,
    pub csv_href: String,
}

/// GET /partner
pub async fn show(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<DemandQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let own = state.usecases.partner.own_report(&actor, q.focus()).await?;
    let area = focus_value(q.focus());
    let page = PartnerPage {
        areas: area_options(state.locale(), &own.report),
        csv_href: with_query(&url("partner_report_csv", &[])?, &[("area", &area)]),
        action: url("partner_report", &[])?,
        area,
        own,
    };
    render_page(&state, &shell, "pages/partner_report.html", &page)
}

/// GET /partner/report.csv
pub async fn csv(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<DemandQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let own = state.usecases.partner.own_report(&actor, q.focus()).await?;
    Ok(csv_response(state.locale(), &own.report, PARTNER_CSV_NAME))
}
