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
    labels::{sector_options, CodeOption},
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
pub const PARTNER_CSV_NAME: &str = "bao-cao-doi-tac.csv";

fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("partner links: {e}")))
}

#[derive(Serialize)]
pub struct PartnerPage {
    pub own: PartnerReportDto,
    /// Every sector, to name a row's.
    pub sectors: Vec<CodeOption>,
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
    let (own, sectors) = tokio::try_join!(
        state.usecases.partner.own_report(&actor, q.focus()),
        state.usecases.sector.named(),
    )?;
    let area = focus_value(q.focus());
    let page = PartnerPage {
        sectors: sector_options(&sectors),
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
    let (own, sectors) = tokio::try_join!(
        state.usecases.partner.own_report(&actor, q.focus()),
        state.usecases.sector.named(),
    )?;
    Ok(csv_response(
        state.locale(),
        &sectors,
        &own.report,
        PARTNER_CSV_NAME,
    ))
}
