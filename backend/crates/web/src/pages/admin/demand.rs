//! /admin/demand and /admin/demand.csv: the demand report (leads by
//! category × week for one area or all), the figures to take to partners.
//! The area is a GET filter (`?area=` a province `p82`, a ward `w28439`,
//! `none`, or empty for all), so the page works without JS; the CSV is the same table to open in
//! a spreadsheet.

use application::{
    admin_units::AreaKey,
    outputs::{CodedDto, DemandAreaDto, DemandReportDto, DemandRowDto, Figure},
    usecases::demand_usecase::LEVEL_PROVINCE,
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
    labels::{sector_name, sector_options, CodeOption},
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

/// One area of the filter (`molecules/area_filter.html`).
#[derive(Debug, Serialize)]
pub struct AreaChoice {
    pub value: String,
    /// As listed under its province: "Cả tỉnh Đồng Tháp", "Phường Cai Lậy".
    pub name: String,
    /// The full name, shown once chosen: "Đồng Tháp — Phường Cai Lậy".
    pub label: String,
}

/// A province's areas (the whole province, then its wards with leads by
/// name), or, with no `province`, "no area" on its own.
#[derive(Debug, Serialize)]
pub struct AreaChoiceGroup {
    pub province: Option<String>,
    pub choices: Vec<AreaChoice>,
}

#[derive(Serialize)]
pub struct DemandPage {
    pub report: DemandReportDto,
    /// Every sector, to name a row's.
    pub sectors: Vec<CodeOption>,
    pub area: String,
    pub areas: Vec<AreaChoiceGroup>,
    /// The area list as shown (`grouped_areas`): every province, the wards
    /// of the one in focus; how many wards each province holds; and each
    /// shown row's own report, in that order.
    pub shown_areas: Vec<DemandAreaDto>,
    pub ward_counts: Vec<usize>,
    pub area_hrefs: Vec<String>,
    pub csv_href: String,
    pub action: String,
}

/// The area's name, "no area" for the businesses without one.
fn area_label(locale: &Locale, label: Option<&str>) -> String {
    label.map_or_else(|| locale.text("demand.unplaced"), str::to_owned)
}

/// The area filter's options, in the report's order: each province as a
/// group, the whole province first, then the wards listed under it, each by
/// its own name. An area of no province ("no area", a partner's lone ward)
/// is a group of its own, by its full name.
pub fn area_options(locale: &Locale, areas: &[DemandAreaDto]) -> Vec<AreaChoiceGroup> {
    let mut groups: Vec<AreaChoiceGroup> = Vec::new();
    for a in areas {
        let label = area_label(locale, a.label.as_deref());
        let province = (a.level == LEVEL_PROVINCE).then(|| label.clone());
        let name = match (&province, &a.name) {
            (Some(p), _) => locale.text_with("partner.whole_province", &[("name", p)]),
            (None, Some(ward)) if a.nested => ward.clone(),
            _ => label.clone(),
        };
        let choice = AreaChoice {
            value: a.value.clone(),
            name,
            label,
        };
        match groups.last_mut() {
            Some(g) if a.nested && g.province.is_some() => g.choices.push(choice),
            _ => groups.push(AreaChoiceGroup {
                province,
                choices: vec![choice],
            }),
        }
    }
    groups
}

/// Each area's own report (`?area=`), in the areas' order: what the area
/// list links to.
pub fn area_hrefs(areas: &[DemandAreaDto]) -> Result<Vec<String>, Error> {
    let report = url("admin_demand", &[])?;
    Ok(areas
        .iter()
        .map(|a| with_query(&report, &[("area", &a.value)]))
        .collect())
}

/// The area list as the page shows it: the top rows (provinces, "no area")
/// always, the wards only under the province in focus (itself, or one of
/// its wards), so the list stays one row per province however many wards
/// have leads. Each top row says how many wards it holds (`ward_counts`, in
/// the same order; 0 for a ward). Every row is the report's own, so nothing
/// the report hides shows here.
pub fn grouped_areas(areas: &[DemandAreaDto], focus: &str) -> (Vec<DemandAreaDto>, Vec<usize>) {
    // Each top row with the wards listed right under it.
    let mut groups: Vec<(&DemandAreaDto, Vec<&DemandAreaDto>)> = Vec::new();
    for a in areas {
        match groups.last_mut() {
            Some((_, wards)) if a.nested => wards.push(a),
            _ => groups.push((a, Vec::new())),
        }
    }
    let mut shown = Vec::with_capacity(groups.len());
    let mut counts = Vec::with_capacity(groups.len());
    for (top, wards) in groups {
        let open =
            !focus.is_empty() && (top.value == focus || wards.iter().any(|w| w.value == focus));
        shown.push(top.clone());
        counts.push(wards.len());
        if open {
            for w in wards {
                shown.push(w.clone());
                counts.push(0);
            }
        }
    }
    (shown, counts)
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
    let (report, sectors) = tokio::try_join!(
        state.usecases.demand.report(&actor, q.focus()),
        state.usecases.sector.named(),
    )?;
    let areas = area_options(state.locale(), &report.areas);
    let path = url("admin_demand", &[])?;
    let area = focus_value(q.focus());
    let (shown_areas, ward_counts) = grouped_areas(&report.areas, &area);
    let area_hrefs = area_hrefs(&shown_areas)?;
    let body = DemandPage {
        csv_href: with_query(&url("admin_demand_csv", &[])?, &[("area", &area)]),
        area,
        areas,
        area_hrefs,
        shown_areas,
        ward_counts,
        report,
        sectors: sector_options(&sectors),
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
    let (report, sectors) = tokio::try_join!(
        state.usecases.demand.report(&actor, q.focus()),
        state.usecases.sector.named(),
    )?;
    Ok(csv_response(
        state.locale(),
        &sectors,
        &report,
        DEMAND_CSV_NAME,
    ))
}

/// The report as CSV: a header, one line per category, then the sums.
/// Excel reads UTF-8 only after a byte order mark.
pub fn csv_response(
    locale: &Locale,
    sectors: &[CodedDto],
    report: &DemandReportDto,
    filename: &str,
) -> Response {
    let few = locale.text_with("demand.fewer_than", &[("min", &report.min_count)]);
    let cell = |f: &Figure| match (f.value, f.at_least) {
        (Some(v), _) => v.to_string(),
        (None, Some(n)) => locale.text_with("demand.at_least", &[("count", &n)]),
        (None, None) => few.clone(),
    };
    let mut header_row = vec![locale.text("demand.category"), locale.text("sector.label")];
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
        let sector = r
            .sector
            .as_deref()
            .map(|s| sector_name(sectors, s))
            .unwrap_or_default();
        let mut fields = vec![name, sector];
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
    let mut searches = vec![locale.text("demand.searches"), String::new()];
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
    // A spreadsheet runs a cell that starts like a formula; a leading
    // apostrophe makes it text. The figures are digits or blank, so only a
    // name an admin typed can start this way.
    let value = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
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
        // A name that would run as a formula is made text.
        assert_eq!(csv_field("=HYPERLINK(1)"), "'=HYPERLINK(1)");
        assert_eq!(csv_field("@a, b"), "\"'@a, b\"");
    }

    #[test]
    fn the_area_list_shows_every_province_and_the_wards_of_the_one_in_focus() {
        let area = |value: &str, nested: bool| DemandAreaDto {
            value: value.into(),
            level: if nested { "ward" } else { "province" }.into(),
            label: Some(value.into()),
            name: Some(value.into()),
            nested,
            weeks: Vec::new(),
            total: Figure {
                value: Some(0),
                at_least: None,
            },
            weeks_at_bar: 0,
            steady: false,
        };
        let report = [
            area("p82", false),
            area("w28439", true),
            area("w28441", true),
            area("p91", false),
            area("w30001", true),
            area("none", false),
        ];
        let shown = |focus: &str| {
            let (areas, counts) = grouped_areas(&report, focus);
            areas
                .iter()
                .map(|a| a.value.clone())
                .zip(counts)
                .collect::<Vec<_>>()
        };
        let row = |v: &str, n: usize| (v.to_owned(), n);
        // All areas: the provinces only, each with how many wards it holds.
        assert_eq!(
            shown(""),
            vec![row("p82", 2), row("p91", 1), row("none", 0)]
        );
        // A province, or one of its wards, opens that province only.
        let open = vec![
            row("p82", 2),
            row("w28439", 0),
            row("w28441", 0),
            row("p91", 1),
            row("none", 0),
        ];
        assert_eq!(shown("p82"), open);
        assert_eq!(shown("w28441"), open);
        assert_eq!(
            shown("none"),
            vec![row("p82", 2), row("p91", 1), row("none", 0)]
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn the_area_filter_groups_wards_under_their_province() {
        let en = Locale::en().expect("en.toml parses");
        let area = |value: &str, level: &str, label: Option<&str>, name: Option<&str>, nested| {
            DemandAreaDto {
                value: value.into(),
                level: level.into(),
                label: label.map(Into::into),
                name: name.map(Into::into),
                nested,
                weeks: Vec::new(),
                total: Figure {
                    value: Some(12),
                    at_least: None,
                },
                weeks_at_bar: 0,
                steady: false,
            }
        };
        let report = [
            area("p82", "province", Some("Dong Thap"), None, false),
            area(
                "w28439",
                "ward",
                Some("Dong Thap - Cai Lay"),
                Some("Cai Lay"),
                true,
            ),
            // A partner's ward whose province is not its own: alone.
            area(
                "w30001",
                "ward",
                Some("An Giang - My Hoa"),
                Some("My Hoa"),
                false,
            ),
            area("none", "none", None, None, false),
        ];
        // Each group as "province: value name / full name, …".
        let seen: Vec<String> = area_options(&en, &report)
            .iter()
            .map(|g| {
                let choices: Vec<String> = g
                    .choices
                    .iter()
                    .map(|c| format!("{} {} / {}", c.value, c.name, c.label))
                    .collect();
                format!(
                    "{}: {}",
                    g.province.as_deref().unwrap_or("-"),
                    choices.join(", ")
                )
            })
            .collect();
        let unplaced = en.text("demand.unplaced");
        assert_eq!(
            seen,
            vec![
                "Dong Thap: p82 All of Dong Thap / Dong Thap, w28439 Cai Lay / Dong Thap - Cai Lay"
                    .to_owned(),
                "-: w30001 An Giang - My Hoa / An Giang - My Hoa".to_owned(),
                format!("-: none {unplaced} / {unplaced}"),
            ]
        );
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
