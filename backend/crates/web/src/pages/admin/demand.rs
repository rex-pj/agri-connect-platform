//! /admin/demand and /admin/demand.csv: the demand report (leads by
//! category × week for one area or all), the figures to take to partners.
//! The area is a GET filter (`?area=` a province `p82`, a ward `w28439`,
//! `none`, or empty for all), so the page works without JS; the CSV is the same table to open in
//! a spreadsheet.

use application::{
    admin_units::AreaKey,
    outputs::{DemandReportDto, DemandRowDto, Figure},
};
use axum::{
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use domain::Error;
use serde::Serialize;

use crate::{
    app_state::AppState,
    extract::ApiQuery,
    locale::Locale,
    middleware::auth::Viewer,
    pages::{
        admin::{render_admin, url},
        listing::with_query,
        see_other,
    },
    requests::DemandQuery,
};

/// The file name the CSV downloads as.
pub const DEMAND_CSV_NAME: &str = "bao-cao-nhu-cau.csv";

#[derive(Serialize)]
pub struct AreaOption {
    pub value: String,
    pub label: String,
}

#[derive(Serialize)]
pub struct DemandPage {
    pub report: DemandReportDto,
    pub area: String,
    pub areas: Vec<AreaOption>,
    pub csv_href: String,
    pub action: String,
}

/// The area's name, "no area" for the businesses without one.
fn area_label(locale: &Locale, label: Option<&str>) -> String {
    label.map_or_else(|| locale.text("demand.unplaced"), str::to_owned)
}

/// The area filter's options: the report's areas, named.
pub fn area_options(locale: &Locale, report: &DemandReportDto) -> Vec<AreaOption> {
    report
        .areas
        .iter()
        .map(|a| AreaOption {
            value: a.value.clone(),
            label: area_label(locale, a.label.as_deref()),
        })
        .collect()
}

/// The filter's value for the area in focus; empty for all of them.
pub fn focus_value(focus: Option<AreaKey>) -> String {
    focus.map_or_else(String::new, |f| f.value())
}

/// GET /admin/demand
pub async fn show(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<DemandQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let report = state.usecases.demand.report(&actor, q.focus()).await?;
    let areas = area_options(state.locale(), &report);
    let path = url("admin_demand", &[])?;
    let area = focus_value(q.focus());
    let body = DemandPage {
        csv_href: with_query(&url("admin_demand_csv", &[])?, &[("area", &area)]),
        area,
        areas,
        report,
        action: path.clone(),
    };
    render_admin(
        &state,
        &actor,
        &path,
        StatusCode::OK,
        "pages/admin/demand.html",
        &body,
    )
}

/// GET /admin/demand.csv — the same table, one row per category.
pub async fn csv(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<DemandQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let report = state.usecases.demand.report(&actor, q.focus()).await?;
    Ok(csv_response(state.locale(), &report, DEMAND_CSV_NAME))
}

/// The report as CSV: a header, one line per category, then the sums.
/// Excel reads UTF-8 only after a byte order mark.
pub fn csv_response(locale: &Locale, report: &DemandReportDto, filename: &str) -> Response {
    let few = locale.text_with("demand.fewer_than", &[("min", &report.min_count)]);
    let cell = |f: &Figure| f.value.map_or_else(|| few.clone(), |v| v.to_string());
    let mut header_row = vec![locale.text("demand.category")];
    for w in &report.weeks {
        header_row.push(w.starts_on.format("%d/%m/%Y").to_string());
    }
    for key in [
        "demand.total",
        "demand.calls",
        "demand.zalo",
        "demand.inquiries",
        "demand.people",
    ] {
        header_row.push(locale.text(key));
    }
    let line = |name: String, r: &DemandRowDto| -> Vec<String> {
        let mut fields = vec![name];
        fields.extend(r.weeks.iter().map(cell));
        fields.extend([&r.total, &r.calls, &r.zalo, &r.inquiries, &r.people].map(cell));
        fields
    };
    let mut lines = vec![header_row];
    for r in &report.rows {
        let name = r
            .category
            .clone()
            .unwrap_or_else(|| locale.text("demand.general"));
        lines.push(line(name, r));
    }
    lines.push(line(locale.text("demand.all"), &report.all));
    let mut searches = vec![locale.text("demand.searches")];
    searches.extend(report.searches.iter().map(cell));
    lines.push(searches);
    let mut body = String::from('\u{feff}');
    for fields in lines {
        let quoted: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
        body.push_str(&quoted.join(","));
        body.push_str("\r\n");
    }
    let mut response = body.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    response
}

/// One CSV field: quoted when it holds a comma, a quote or a line break.
fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_fields_are_quoted_only_when_needed() {
        assert_eq!(csv_field("Phun thuoc"), "Phun thuoc");
        assert_eq!(csv_field("a, b"), "\"a, b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn the_area_filter_reads_provinces_wards_and_none() {
        let q = |s: &str| DemandQuery { area: s.into() }.focus();
        assert_eq!(q(""), None);
        assert_eq!(q("p82"), Some(AreaKey::Province("82".into())));
        assert_eq!(q("w28439"), Some(AreaKey::Ward("28439".into())));
        assert_eq!(q("none"), Some(AreaKey::Unplaced));
        assert_eq!(q("12"), None);
        assert_eq!(focus_value(q("w28439")), "w28439");
    }
}
