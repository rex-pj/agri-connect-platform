//! /info and /info/{code}: what the platform is and the rules it keeps
//! (the terms, the policies, the legal entity, the contact page), as the
//! admin wrote them (`application::usecases::info_page_usecase`).
//!
//! A page shows while it is on, or in preview to an account that may
//! preview it (with the preview strip, never indexed), and only once a
//! version of its text is in force. An earlier version is read with
//! `?version=`, said to be no longer in force, never indexed.

use application::{info_pages::FOOTER_GROUPS, outputs::InfoLinkDto};
use axum::{
    extract::State,
    http::{HeaderName, HeaderValue},
    response::Response,
};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    extract::{ApiPath, ApiQuery},
    pages::parse_id,
    routes,
    shell::Shell,
    views::render_page,
};

const ROBOTS: HeaderName = HeaderName::from_static("x-robots-tag");
const NOINDEX: HeaderValue = HeaderValue::from_static("noindex");

fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("info links: {e}")))
}

/// One group of the hub and the footer: its code (its title's key) and its
/// pages.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InfoGroup {
    pub code: &'static str,
    pub links: Vec<InfoLink>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InfoLink {
    pub href: String,
    pub title: String,
    pub icon: Option<String>,
    pub preview: bool,
}

/// The pages a visitor sees, by footer group, each group only with a page.
pub fn grouped(pages: Vec<InfoLinkDto>) -> Result<Vec<InfoGroup>, Error> {
    let mut groups: Vec<InfoGroup> = FOOTER_GROUPS
        .into_iter()
        .map(|code| InfoGroup {
            code,
            links: Vec::new(),
        })
        .collect();
    for page in pages {
        let href = url("info_page", &[("code", &page.code)])?;
        if let Some(group) = groups.iter_mut().find(|g| g.code == page.footer_group) {
            group.links.push(InfoLink {
                href,
                title: page.title,
                icon: page.icon,
                preview: page.preview,
            });
        }
    }
    groups.retain(|g| !g.links.is_empty());
    Ok(groups)
}

#[derive(Serialize)]
struct HubPage {
    groups: Vec<InfoGroup>,
}

/// GET /info
pub async fn hub(shell: Shell, State(state): State<AppState>) -> Result<Response, Error> {
    let pages = state.usecases.info_page.shown(shell.may_preview()).await?;
    let page = HubPage {
        groups: grouped(pages)?,
    };
    render_page(&state, &shell, "pages/info_hub.html", &page)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct VersionQuery {
    /// An earlier version's id.
    pub version: String,
}

/// One version in the page's list.
#[derive(Serialize)]
struct VersionLink {
    href: String,
    effective_on: chrono::NaiveDate,
    change_note: Option<String>,
    /// The one shown.
    current: bool,
    /// The one in force.
    in_force: bool,
}

#[derive(Serialize)]
struct InfoPageView {
    page: application::outputs::InfoPageViewDto,
    /// The version in force, at the page's own address.
    current_href: String,
    /// Each version with its address: the one in force is the page's own.
    versions: Vec<VersionLink>,
    hub_href: String,
    /// Which version is shown, counted from the first, and how many.
    version_number: usize,
    version_count: usize,
    /// Its footer group (`about`, `policy`) and the other pages of it.
    group: String,
    others: Vec<InfoLink>,
}

/// GET /info/{code}
pub async fn show(
    mut shell: Shell,
    State(state): State<AppState>,
    ApiPath(code): ApiPath<String>,
    ApiQuery(q): ApiQuery<VersionQuery>,
) -> Result<Response, Error> {
    let version = match q.version.trim() {
        "" => None,
        raw => Some(parse_id(raw, ErrorCode::InfoPageNotFound)?),
    };
    let pages = &state.usecases.info_page;
    let (page, shown) = tokio::try_join!(
        pages.page(&code, shell.may_preview(), version),
        pages.shown(shell.may_preview()),
    )?;
    // Its group, and the other pages of it.
    let group = shown
        .iter()
        .find(|p| p.code == page.code)
        .map(|p| p.footer_group.clone())
        .unwrap_or_default();
    let others = grouped(
        shown
            .into_iter()
            .filter(|p| p.footer_group == group && p.code != page.code)
            .collect(),
    )?
    .into_iter()
    .flat_map(|g| g.links)
    .collect();
    if page.preview {
        shell.preview = Some(url("admin_info_page", &[("id", &page.id.to_string())])?);
    }
    let current_href = url("info_page", &[("code", &page.code)])?;
    let versions = page
        .versions
        .iter()
        .enumerate()
        .map(|(i, v)| VersionLink {
            href: if i == 0 {
                current_href.clone()
            } else {
                format!("{current_href}?version={}", v.id)
            },
            effective_on: v.effective_on,
            change_note: v.change_note.clone(),
            current: v.current,
            in_force: i == 0,
        })
        .collect();
    let noindex = page.preview || page.superseded_on.is_some();
    // Which version this is, counted from the first: "version 3 of 3".
    let total = page.versions.len();
    let number = page
        .versions
        .iter()
        .position(|v| v.current)
        .map_or(total, |i| total - i);
    let view = InfoPageView {
        version_number: number,
        version_count: total,
        group,
        others,
        page,
        current_href,
        versions,
        hub_href: url("info_pages", &[])?,
    };
    let mut response = render_page(&state, &shell, "pages/info_page.html", &view)?;
    if noindex {
        response.headers_mut().insert(ROBOTS, NOINDEX);
    }
    Ok(response)
}
