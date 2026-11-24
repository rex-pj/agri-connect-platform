//! GET /search?q=…&tab=… — one box, three result kinds: services, products
//! and providers (shops, found by their name or by what they offer).
//!
//! Tabs are links (`tab=services`), so they work without JS; the server marks
//! the other sections `hidden`. A page of several lists pages each on its
//! own: the "all" tab previews every kind with a "see more" link to that
//! kind's tab, and a kind's tab (or the one kind found) is its whole list a
//! page at a time, its page links carrying the tab (`listing::tab_page`).
//! Since a tab's content differs from its preview, tabs always load.
//!
//! Only what was found is shown: a kind with no result has no section and no
//! tab (its tab in an old link shows everything), one kind alone needs no
//! tabs, and nothing at all is one message with the ways forward.
//!
//! What was typed is searched in every sector, whatever the sector cookie
//! says; `sector=` (the links over the results) narrows this one search and
//! is never remembered.

use crate::{middleware::client_ip::ClientIp, pages::visitor};
use application::{
    features::Feature,
    inputs::{BrowseParams, SearchInput, SearchPlace},
    outputs::{BrowseResponse, ProductDto, ServiceBrowseResponse},
};
use axum::{extract::State, response::Response, Extension};
use domain::{
    models::{
        business::ProviderSort,
        product::{ProductFilter, ProductSort},
    },
    Error,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::SEARCH_PREVIEW_COUNT,
    extract::ApiQuery,
    pages::{
        cards::{rated_providers, rated_services, ProviderCard, ServiceCard},
        listing::{
            parse_opt, sector_links, tab_page, typed_sector, with_query, Pagination, SectorLink,
        },
        see_other,
    },
    routes,
    shell::Shell,
    views::render_page,
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SearchQuery {
    pub q: String,
    pub tab: String,
    /// One sector's results; blank or unknown: every sector's.
    pub sector: String,
    /// The page of the chosen tab's list.
    pub page: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tab {
    All,
    Services,
    Products,
    Providers,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::All, Tab::Services, Tab::Products, Tab::Providers];

    const fn as_str(self) -> &'static str {
        match self {
            Tab::All => "all",
            Tab::Services => "services",
            Tab::Products => "products",
            Tab::Providers => "providers",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Tab::All => "search.tab_all",
            Tab::Services => "nav.services",
            Tab::Products => "nav.products",
            Tab::Providers => "nav.shops",
        }
    }

    fn parse(raw: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|t| t.as_str() == raw.trim())
            .unwrap_or(Tab::All)
    }
}

/// One link of `molecules/tab_nav.html`; `tab` names the panel it shows.
#[derive(Debug, Serialize)]
pub struct TabLink {
    pub tab: &'static str,
    pub label: String,
    pub href: String,
    pub count: Option<i64>,
    pub active: bool,
}

#[derive(Serialize)]
pub struct SearchPage {
    pub q: String,
    /// Everything found, of every kind: none is the page's one empty state.
    pub total: i64,
    /// The sector the search is narrowed to, by name.
    pub sector_name: Option<String>,
    /// The same search in every sector: the way out of an empty sector.
    pub every_sector_href: String,
    pub tab: &'static str,
    /// The kinds with a result, after "all"; none unless two kinds have one.
    pub tabs: Vec<TabLink>,
    /// The sectors to narrow the results to, "every sector" first; none
    /// while no sector is shown.
    pub sectors: Vec<SectorLink>,
    /// A preview (the "all" tab) or one page of the kind's whole list.
    pub services: Vec<ServiceCard>,
    pub service_total: i64,
    /// "Xem thêm" under a preview with more: this kind's tab of this search.
    pub services_more: Option<String>,
    /// The page links of the kind's whole list, on its own tab.
    pub service_pages: Option<Pagination>,
    pub products: Vec<ProductDto>,
    pub product_total: i64,
    pub products_more: Option<String>,
    pub product_pages: Option<Pagination>,
    pub providers: Vec<ProviderCard>,
    pub provider_total: i64,
    pub providers_more: Option<String>,
    pub provider_pages: Option<Pagination>,
}

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("search links: {e}")))
}

/// The results of one search: of each kind, the page `page_of` it says.
struct Found {
    services: ServiceBrowseResponse,
    products: application::outputs::ProductBrowseResponse,
    providers: BrowseResponse,
}

async fn find(
    state: &AppState,
    shell: &Shell,
    q: &str,
    sector: Option<&str>,
    page_of: impl Fn(Tab) -> i64,
) -> Result<Found, Error> {
    let explore = &state.usecases.explore;
    let text = |page: i64| BrowseParams {
        service_id: None,
        sector: sector.map(str::to_owned),
        name: Some(q.to_owned()),
        sort: ProviderSort::default(),
        page,
    };
    // A kind of a feature this visitor does not see is not searched: no
    // result, so no tab, no section.
    let (services, products, providers) = tokio::try_join!(
        explore.browse_services(text(page_of(Tab::Services))),
        async {
            if !shell.shows(Feature::Supplies) {
                return Ok(Default::default());
            }
            let filter = ProductFilter {
                name: Some(q.to_owned()),
                sector: sector.map(str::to_owned),
                ..ProductFilter::default()
            };
            let page = page_of(Tab::Products);
            (state.usecases.product)
                .browse_products(filter, ProductSort::default(), page)
                .await
        },
        async {
            match shell.shows(Feature::Directory) {
                true => {
                    explore
                        .browse_providers(text(page_of(Tab::Providers)))
                        .await
                }
                false => Ok(Default::default()),
            }
        },
    )?;
    Ok(Found {
        services,
        products,
        providers,
    })
}

pub async fn show(
    shell: Shell,
    ip: Option<Extension<ClientIp>>,
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<SearchQuery>,
) -> Result<Response, Error> {
    let q = query.q.trim().to_owned();
    let tab = Tab::parse(&query.tab);
    let search_url = url("search")?;
    let sector = typed_sector(&shell.sectors, &query.sector, &q);
    let sector = sector.as_deref();
    let tab_param = |t: Tab| if t == Tab::All { "" } else { t.as_str() };

    // `page` belongs to the tab asked for; every other kind opens on page 1.
    let page_no = parse_opt::<i64>(&query.page).unwrap_or(1);
    let page_of = |t: Tab| tab_page(tab.as_str(), t.as_str(), page_no);
    let found = if q.is_empty() {
        None
    } else {
        Some(find(&state, &shell, &q, sector, page_of).await?)
    };
    let service_total = found.as_ref().map_or(0, |f| f.services.total);
    let product_total = found.as_ref().map_or(0, |f| f.products.total);
    let provider_total = found.as_ref().map_or(0, |f| f.providers.total);
    // Only the first visit of a search counts, not a tab switch nor a
    // narrowing to one sector: the demand report takes what all sectors hold.
    if found.is_some() && query.tab.trim().is_empty() && sector.is_none() {
        state.usecases.demand.record_search(SearchInput {
            query: q.clone(),
            category_id: None,
            place: SearchPlace::Anywhere,
            results: service_total + product_total + provider_total,
            visitor: visitor(&shell, ip.as_ref().map(|e| &e.0)),
        });
    }
    // This search on one kind's tab (its `page`, when past the first).
    let tab_href = |t: Tab, page: i64| {
        let page = if page > 1 {
            page.to_string()
        } else {
            String::new()
        };
        with_query(
            &search_url,
            &[
                ("q", q.as_str()),
                ("tab", tab_param(t)),
                ("sector", sector.unwrap_or_default()),
                ("page", &page),
            ],
        )
    };

    let count = |t: Tab| match t {
        Tab::All => None,
        Tab::Services => Some(service_total),
        Tab::Products => Some(product_total),
        Tab::Providers => Some(provider_total),
    };
    let kinds = Tab::ALL
        .into_iter()
        .filter(|t| count(*t).is_some_and(|n| n > 0))
        .count();
    // A kind with nothing has no tab, and one kind alone needs none.
    let tab = if kinds > 1 && count(tab) != Some(0) {
        tab
    } else {
        Tab::All
    };
    let tabs = Tab::ALL
        .into_iter()
        .filter(|t| kinds > 1 && count(*t) != Some(0))
        .map(|t| TabLink {
            tab: t.as_str(),
            label: state.locale().text(t.label()),
            href: tab_href(t, 1),
            count: count(t),
            active: t == tab,
        })
        .collect();
    // The kind shown whole, a page at a time: the chosen tab's, or the one
    // kind found. On the "all" tab of several kinds, each is a preview.
    let whole = match tab {
        Tab::All if kinds == 1 => Tab::ALL
            .into_iter()
            .find(|t| count(*t).is_some_and(|n| n > 0)),
        Tab::All => None,
        t => Some(t),
    };
    // A kind's own page links, or under a preview with more, its "see more".
    let paging = |t: Tab, page: i64, total_pages: i64, total: i64| {
        if whole == Some(t) {
            let pages = Pagination::new(
                &search_url,
                &[
                    ("q", q.as_str()),
                    ("tab", tab_param(t)),
                    ("sector", sector.unwrap_or_default()),
                ],
                page,
                total_pages,
            );
            (None, pages)
        } else {
            let more = (total > SEARCH_PREVIEW_COUNT as i64).then(|| tab_href(t, 1));
            (more, None)
        }
    };
    // A preview keeps the first few; a whole list keeps its page.
    let shown = |t: Tab| {
        if whole == Some(t) {
            usize::MAX
        } else {
            SEARCH_PREVIEW_COUNT
        }
    };

    let mut paged = [(None, None), (None, None), (None, None)];
    let (services, products, providers) = match found {
        Some(f) => {
            if let Some(t) = whole {
                let (page, total_pages) = match t {
                    Tab::Products => (f.products.page, f.products.total_pages),
                    Tab::Providers => (f.providers.page, f.providers.total_pages),
                    _ => (f.services.page, f.services.total_pages),
                };
                if page > total_pages && total_pages >= 1 {
                    return see_other(&tab_href(t, total_pages));
                }
            }
            paged = [
                paging(
                    Tab::Services,
                    f.services.page,
                    f.services.total_pages,
                    service_total,
                ),
                paging(
                    Tab::Products,
                    f.products.page,
                    f.products.total_pages,
                    product_total,
                ),
                paging(
                    Tab::Providers,
                    f.providers.page,
                    f.providers.total_pages,
                    provider_total,
                ),
            ];
            (
                rated_services(
                    &state,
                    &shell,
                    f.services
                        .items
                        .into_iter()
                        .take(shown(Tab::Services))
                        .collect(),
                )
                .await?,
                f.products
                    .items
                    .into_iter()
                    .take(shown(Tab::Products))
                    .collect(),
                rated_providers(
                    &state,
                    &shell,
                    f.providers
                        .items
                        .into_iter()
                        .take(shown(Tab::Providers))
                        .map(ProviderCard::from)
                        .collect(),
                )
                .await?,
            )
        }
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    let [(services_more, service_pages), (products_more, product_pages), (providers_more, provider_pages)] =
        paged;

    let sectors = sector_links(
        &shell.sectors,
        &search_url,
        &[("q", q.as_str()), ("tab", tab_param(tab))],
        sector,
    );

    let page = SearchPage {
        total: service_total + product_total + provider_total,
        sector_name: shell
            .sectors
            .iter()
            .find(|s| sector == Some(s.value.as_str()))
            .map(|s| s.label.clone()),
        every_sector_href: with_query(&search_url, &[("q", q.as_str())]),
        sectors,
        q,
        tab: tab.as_str(),
        tabs,
        services,
        service_total,
        services_more,
        service_pages,
        products,
        product_total,
        products_more,
        product_pages,
        providers,
        provider_total,
        providers_more,
        provider_pages,
    };
    render_page(&state, &shell, "pages/search.html", &page)
}

#[cfg(test)]
mod tests {
    use super::Tab;

    #[test]
    fn unknown_tabs_show_everything() {
        assert_eq!(Tab::parse("services"), Tab::Services);
        assert_eq!(Tab::parse(" products "), Tab::Products);
        assert_eq!(Tab::parse("providers"), Tab::Providers);
        assert_eq!(Tab::parse("ads"), Tab::All);
        assert_eq!(Tab::parse(""), Tab::All);
    }
}
