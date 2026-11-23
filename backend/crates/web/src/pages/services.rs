//! GET /services — every live service, searchable and filterable by
//! category — and GET /services/category/{id}/{slug} — one category's
//! services. Each card is the service itself (photo, name, price) with the
//! provider who offers it and a call button; the provider directory is
//! `/providers` (`directory`).
//!
//! Plain GET forms and links, like the product lists.

use crate::{middleware::client_ip::ClientIp, pages::visitor};
use application::{
    inputs::{BrowseParams, SearchInput, SearchPlace, Visitor},
    outputs::CategoryDto,
};
use axum::{extract::State, response::Response, Extension};
use domain::{constants::CATEGORY_TYPE_SERVICE, models::business::ProviderSort, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::{km, EXPLORE_DEFAULT_RADIUS_M},
    extract::{ApiPath, ApiQuery},
    locale::Locale,
    pages::{
        cards::{rated_services, ServiceCard},
        explore::TAB_PROVIDERS,
        listing::{
            category_sub, chips, find_category, in_sector, list_sector, parse_opt, past_the_end,
            redirect_to_canonical, sector_links, typed_sector, with_query, FilterForm, Pagination,
            SectorLink, SortChip,
        },
        parse_id,
        search::TabLink,
    },
    routes,
    shell::{SectorChoice, Shell},
    views::render_page,
};

/// Everything as text: a hand-edited URL must give a page, not a 400.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub q: String,
    pub service_id: String,
    pub sort: String,
    pub page: String,
    /// One sector's results of a typed search (`typed_sector`).
    pub sector: String,
}

/// A list page's search text, service-category filter, sort and page, as
/// parsed from its query; shared by the service listing and the provider
/// directory, whose forms are the same.
pub(super) struct ListState {
    pub search: String,
    pub service_id: Option<i32>,
    pub sort: ProviderSort,
    pub page: i64,
    /// The sector the typed search is narrowed to by its own `sector=`.
    pub sector: Option<String>,
}

impl ListState {
    /// The search this list is, for the demand report: its words and
    /// category on the first page (later pages are the same search, and so
    /// is narrowing it to a sector: the report takes what all sectors hold).
    pub fn search(&self, results: i64, visitor: Visitor) -> Option<SearchInput> {
        (self.page <= 1
            && self.sector.is_none()
            && (!self.search.is_empty() || self.service_id.is_some()))
        .then(|| SearchInput {
            query: self.search.clone(),
            category_id: self.service_id,
            place: SearchPlace::Anywhere,
            results,
            visitor,
        })
    }

    /// A category that is not among `categories` (hidden, or a product's) is
    /// no filter at all. `sectors` are the shell's choices.
    pub fn parse(q: &ListQuery, categories: &[CategoryDto], sectors: &[SectorChoice]) -> Self {
        Self {
            sector: typed_sector(sectors, &q.sector, &q.q),
            search: q.q.trim().to_owned(),
            service_id: parse_opt::<i32>(&q.service_id)
                .filter(|id| categories.iter().any(|c| c.id == *id)),
            sort: ProviderSort::parse(Some(q.sort.trim())),
            page: parse_opt(&q.page).unwrap_or(1),
        }
    }

    /// The use case's parameters; `sector` (the visitor's) narrows the
    /// services to that sector's categories and those of every sector, a
    /// typed search only by its own `sector=`.
    pub fn params(&self, sector: Option<&str>) -> BrowseParams {
        BrowseParams {
            service_id: self.service_id,
            sector: self
                .sector
                .clone()
                .or_else(|| list_sector(sector, self.service_id, &self.search)),
            name: (!self.search.is_empty()).then(|| self.search.clone()),
            sort: self.sort,
            page: self.page,
        }
    }

    /// The query parameters that reproduce this list, minus the page.
    pub fn kept(&self) -> Vec<(&'static str, String)> {
        let sort = if self.sort == ProviderSort::default() {
            ""
        } else {
            self.sort.as_str()
        };
        vec![
            ("q", self.search.clone()),
            (
                "service_id",
                self.service_id.map(|id| id.to_string()).unwrap_or_default(),
            ),
            ("sort", sort.to_owned()),
            ("sector", self.sector.clone().unwrap_or_default()),
        ]
    }

    /// The links that narrow a typed search at `path` to one sector; none
    /// without words (the page then shows the visitor's switch).
    pub fn sector_links(&self, sectors: &[SectorChoice], path: &str) -> Vec<SectorLink> {
        if self.search.is_empty() {
            return Vec::new();
        }
        let kept = self.kept();
        let kept: Vec<_> = borrowed(&kept)
            .into_iter()
            .filter(|(k, _)| *k != "sector")
            .collect();
        sector_links(sectors, path, &kept, self.sector.as_deref())
    }

    fn active(&self) -> usize {
        usize::from(self.service_id.is_some()) + usize::from(self.sort != ProviderSort::default())
    }

    pub fn filtered(&self) -> bool {
        !self.search.is_empty() || self.active() > 0
    }

    pub fn form(
        &self,
        locale: &Locale,
        path: &str,
        categories: &[CategoryDto],
        placeholder_key: &str,
    ) -> FilterForm {
        FilterForm {
            reset_href: with_query(
                path,
                &[
                    ("q", self.search.as_str()),
                    ("sector", self.sector.as_deref().unwrap_or_default()),
                ],
            ),
            action: path.to_owned(),
            placeholder: locale.text(placeholder_key),
            q: self.search.clone(),
            category_param: "service_id",
            category_label: locale.text("filters.service_type"),
            category_id: self.service_id,
            categories: chips(categories),
            price: None,
            sort: self.sort.as_str(),
            sorts: ProviderSort::ALL
                .into_iter()
                .map(|s| SortChip {
                    value: s.as_str(),
                    label: locale.text(sort_label(s)),
                })
                .collect(),
            active: self.active(),
            hidden: self.sector.iter().map(|s| ("sector", s.clone())).collect(),
        }
    }
}

/// The two ways to look for someone to do a job: their services
/// (`/services`) or their shops (`/providers`). Both lists read the same
/// query (`ListQuery`), so switching keeps the search, category and sort.
/// Each tab is named after its route.
pub(super) const VIEW_SERVICES: &str = "services";
pub(super) const VIEW_PROVIDERS: &str = "providers";

/// The near-me page on the same view, with the chosen category kept.
pub(super) fn near_me_href(view: &str, service_id: Option<i32>) -> Result<String, Error> {
    let id = service_id.map(|id| id.to_string()).unwrap_or_default();
    let tab = if view == VIEW_PROVIDERS {
        TAB_PROVIDERS
    } else {
        ""
    };
    Ok(with_query(
        &url("explore", &[])?,
        &[("service_id", id.as_str()), ("tab", tab)],
    ))
}

/// The near-me banner's line (`key`), with the radius near-me starts at.
pub(super) fn near_me_sub(locale: &Locale, key: &str) -> String {
    locale.text_with(key, &[("km", &km(EXPLORE_DEFAULT_RADIUS_M))])
}

pub(super) fn view_tabs(
    locale: &Locale,
    active: &str,
    kept: &[(&str, &str)],
) -> Result<Vec<TabLink>, Error> {
    [
        (VIEW_SERVICES, "nav.services"),
        (VIEW_PROVIDERS, "nav.shops"),
    ]
    .into_iter()
    .map(|(tab, label)| {
        Ok(TabLink {
            tab,
            label: locale.text(label),
            href: with_query(&url(tab, &[])?, kept),
            count: None,
            active: tab == active,
        })
    })
    .collect()
}

/// Borrowed pairs for `with_query` and friends.
pub(super) fn borrowed<'a>(pairs: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    pairs.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

fn sort_label(sort: ProviderSort) -> &'static str {
    match sort {
        ProviderSort::VerifiedFirst => "sort.verified_first",
        ProviderSort::Newest => "sort.newest",
        ProviderSort::NameAsc => "sort.name_asc",
    }
}

#[derive(Serialize)]
pub struct ServicesPage {
    pub title: String,
    pub icon: String,
    pub sub: String,
    pub back_href: String,
    pub back_label: String,
    /// The near-me banner's second line.
    pub geo_sub: String,
    pub category: Option<CategoryDto>,
    pub filters: Option<FilterForm>,
    /// A typed search's sector links, in place of the visitor's switch.
    pub sectors: Vec<SectorLink>,
    pub services: Vec<ServiceCard>,
    pub total: i64,
    pub pagination: Option<Pagination>,
    pub filtered: bool,
    /// Services or shops: the same search as either list.
    pub tabs: Vec<TabLink>,
    /// The near-me banner's link (`near_me_href`).
    pub near_href: String,
}

pub(super) fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("services links: {e}")))
}

/// GET /services
pub async fn list(
    shell: Shell,
    ip: Option<Extension<ClientIp>>,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let explore = &state.usecases.explore;
    let categories = explore.get_categories(Some(CATEGORY_TYPE_SERVICE)).await?;
    let list = ListState::parse(&q, &categories, &shell.sectors);
    let result = explore
        .browse_services(list.params(shell.sector.as_deref()))
        .await?;
    if let Some(search) = list.search(result.total, visitor(&shell, ip.as_ref().map(|e| &e.0))) {
        state.usecases.demand.record_search(search);
    }

    let path = url("services", &[])?;
    let kept = list.kept();
    if let Some(redirect) = past_the_end(&path, &borrowed(&kept), result.page, result.total_pages) {
        return redirect;
    }
    let page = ServicesPage {
        title: state.locale().text("services.title"),
        icon: "fa-border-all".to_owned(),
        sub: state.locale().text("services.sub"),
        back_href: url("home", &[])?,
        back_label: state.locale().text("nav.home"),
        geo_sub: near_me_sub(state.locale(), "geo.sub_services"),
        category: None,
        pagination: Pagination::new(&path, &borrowed(&kept), result.page, result.total_pages),
        filtered: list.filtered(),
        sectors: list.sector_links(&shell.sectors, &path),
        filters: Some(list.form(
            state.locale(),
            &path,
            // The chips offer the visitor's sector; a shared link to
            // another sector's category still works (parsed from all).
            &in_sector(&categories, shell.sector.as_deref()),
            "services.search_placeholder",
        )),
        total: result.total,
        services: rated_services(&state, result.items).await?,
        tabs: view_tabs(state.locale(), VIEW_SERVICES, &borrowed(&kept))?,
        near_href: near_me_href(VIEW_SERVICES, list.service_id)?,
    };
    render_page(&state, &shell, "pages/services.html", &page)
}

/// GET /services/category/{id}/{slug}
pub async fn category(
    shell: Shell,
    ip: Option<Extension<ClientIp>>,
    State(state): State<AppState>,
    ApiPath((raw_id, _slug)): ApiPath<(String, String)>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let id = parse_id(&raw_id, ErrorCode::CategoryNotFound)?;
    let explore = &state.usecases.explore;
    let categories = explore.get_categories(Some(CATEGORY_TYPE_SERVICE)).await?;
    let category = find_category(&categories, id)?;
    let canonical = url(
        "service_category",
        &[("id", &id.to_string()), ("slug", &category.slug)],
    )?;
    let page_number: i64 = parse_opt(&q.page).unwrap_or(1);
    if let Some(redirect) = redirect_to_canonical(&shell.path, &canonical, page_number) {
        return redirect;
    }

    let result = explore
        .browse_services(BrowseParams {
            service_id: Some(id),
            // The category is chosen: its sector is no filter.
            sector: None,
            name: None,
            sort: ProviderSort::default(),
            page: page_number,
        })
        .await?;

    if page_number <= 1 {
        state.usecases.demand.record_search(SearchInput {
            query: String::new(),
            category_id: Some(id),
            place: SearchPlace::Anywhere,
            results: result.total,
            visitor: visitor(&shell, ip.as_ref().map(|e| &e.0)),
        });
    }
    if let Some(redirect) = past_the_end(&canonical, &[], result.page, result.total_pages) {
        return redirect;
    }
    let page = ServicesPage {
        title: category.name.clone(),
        icon: category.icon.clone().unwrap_or_else(|| "fa-tag".to_owned()),
        sub: category_sub(state.locale(), &category, result.total, "services.count"),
        back_href: url("services", &[])?,
        back_label: state.locale().text("nav.services"),
        geo_sub: near_me_sub(state.locale(), "geo.sub_category"),
        pagination: Pagination::new(&canonical, &[], result.page, result.total_pages),
        filtered: false,
        sectors: Vec::new(),
        filters: None,
        category: Some(category),
        total: result.total,
        services: rated_services(&state, result.items).await?,
        tabs: view_tabs(
            state.locale(),
            VIEW_SERVICES,
            &[("service_id", &id.to_string())],
        )?,
        near_href: near_me_href(VIEW_SERVICES, Some(id))?,
    };
    render_page(&state, &shell, "pages/services.html", &page)
}
