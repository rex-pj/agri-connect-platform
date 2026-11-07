//! GET /search?q=…&tab=… — one box, three result kinds: services, products
//! and providers (shops, found by their name or by what they offer).
//!
//! Tabs are links (`tab=services`), so they work without JS; the server marks
//! the other sections `hidden`. Alpine `tabs` switches them in place.

use crate::{middleware::client_ip::ClientIp, pages::visitor};
use application::{
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
        listing::with_query,
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
    pub tab: &'static str,
    pub tabs: Vec<TabLink>,
    pub services: Vec<ServiceCard>,
    pub service_total: i64,
    /// "Xem thêm": the full listing with this search, when there is more.
    pub services_more: Option<String>,
    pub products: Vec<ProductDto>,
    pub product_total: i64,
    pub products_more: Option<String>,
    pub providers: Vec<ProviderCard>,
    pub provider_total: i64,
    pub providers_more: Option<String>,
}

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("search links: {e}")))
}

/// The results of one search, each kind's first page.
struct Found {
    services: ServiceBrowseResponse,
    products: application::outputs::ProductBrowseResponse,
    providers: BrowseResponse,
}

async fn find(state: &AppState, q: &str) -> Result<Found, Error> {
    let explore = &state.usecases.explore;
    let text = || BrowseParams {
        service_id: None,
        name: Some(q.to_owned()),
        sort: ProviderSort::default(),
        page: 1,
    };
    let (services, products, providers) = tokio::try_join!(
        explore.browse_services(text()),
        state.usecases.product.browse_products(
            ProductFilter {
                name: Some(q.to_owned()),
                ..ProductFilter::default()
            },
            ProductSort::default(),
            1,
        ),
        explore.browse_providers(text()),
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

    let found = if q.is_empty() {
        None
    } else {
        Some(find(&state, &q).await?)
    };
    let service_total = found.as_ref().map_or(0, |f| f.services.total);
    let product_total = found.as_ref().map_or(0, |f| f.products.total);
    let provider_total = found.as_ref().map_or(0, |f| f.providers.total);
    // Only the first visit of a search counts, not a tab switch.
    if found.is_some() && query.tab.trim().is_empty() {
        state.usecases.demand.record_search(SearchInput {
            query: q.clone(),
            category_id: None,
            place: SearchPlace::Anywhere,
            results: service_total + product_total + provider_total,
            visitor: visitor(&shell, ip.as_ref().map(|e| &e.0)),
        });
    }
    let more = |route: &str, total: i64| -> Result<Option<String>, Error> {
        (total > SEARCH_PREVIEW_COUNT as i64)
            .then(|| url(route).map(|path| with_query(&path, &[("q", q.as_str())])))
            .transpose()
    };

    let tabs = Tab::ALL
        .into_iter()
        .map(|t| TabLink {
            tab: t.as_str(),
            label: state.locale().text(t.label()),
            href: with_query(
                &search_url,
                &[
                    ("q", q.as_str()),
                    ("tab", if t == Tab::All { "" } else { t.as_str() }),
                ],
            ),
            count: match t {
                Tab::All => None,
                Tab::Services => Some(service_total),
                Tab::Products => Some(product_total),
                Tab::Providers => Some(provider_total),
            },
            active: t == tab,
        })
        .collect();

    let (services, products, providers) = match found {
        Some(f) => (
            rated_services(
                &state,
                f.services
                    .items
                    .into_iter()
                    .take(SEARCH_PREVIEW_COUNT)
                    .collect(),
            )
            .await?,
            f.products
                .items
                .into_iter()
                .take(SEARCH_PREVIEW_COUNT)
                .collect(),
            rated_providers(
                &state,
                f.providers
                    .items
                    .into_iter()
                    .take(SEARCH_PREVIEW_COUNT)
                    .map(ProviderCard::from)
                    .collect(),
            )
            .await?,
        ),
        None => (Vec::new(), Vec::new(), Vec::new()),
    };

    let page = SearchPage {
        services_more: more("services", service_total)?,
        products_more: more("products", product_total)?,
        providers_more: more("providers", provider_total)?,
        q,
        tab: tab.as_str(),
        tabs,
        services,
        service_total,
        products,
        product_total,
        providers,
        provider_total,
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
