//! GET /search?q=…&tab=… — one box, two result kinds: providers and products.
//!
//! Tabs are links (`tab=services`), so they work without JS; the server marks
//! the other sections `hidden`. Alpine `tabs` switches them in place.

use application::{inputs::BrowseParams, outputs::ProductDto};
use axum::{extract::State, response::Response};
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
    pages::{cards::ProviderCard, listing::with_query},
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
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::All, Tab::Services, Tab::Products];

    const fn as_str(self) -> &'static str {
        match self {
            Tab::All => "all",
            Tab::Services => "services",
            Tab::Products => "products",
        }
    }

    fn parse(raw: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|t| t.as_str() == raw.trim())
            .unwrap_or(Tab::All)
    }
}

#[derive(Debug, Serialize)]
pub struct TabLink {
    pub tab: &'static str,
    pub label: &'static str,
    pub href: String,
    pub count: Option<i64>,
    pub active: bool,
}

#[derive(Serialize)]
pub struct SearchPage {
    pub q: String,
    pub tab: &'static str,
    pub tabs: Vec<TabLink>,
    pub providers: Vec<ProviderCard>,
    pub provider_total: i64,
    /// "Xem thêm": the full directory with this search, when there is more.
    pub providers_more: Option<String>,
    pub products: Vec<ProductDto>,
    pub product_total: i64,
    pub products_more: Option<String>,
}

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("search links: {e}")))
}

pub async fn show(
    shell: Shell,
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<SearchQuery>,
) -> Result<Response, Error> {
    let q = query.q.trim().to_owned();
    let tab = Tab::parse(&query.tab);
    let search_url = url("search")?;

    let (providers, products) = if q.is_empty() {
        (None, None)
    } else {
        let (p, r) = tokio::try_join!(
            state.usecases.explore.browse_providers(BrowseParams {
                service_id: None,
                name: Some(q.clone()),
                sort: ProviderSort::default(),
                page: 1,
            }),
            state.usecases.product.browse_products(
                ProductFilter {
                    name: Some(q.clone()),
                    ..ProductFilter::default()
                },
                ProductSort::default(),
                1,
            ),
        )?;
        (Some(p), Some(r))
    };
    let provider_total = providers.as_ref().map_or(0, |p| p.total);
    let product_total = products.as_ref().map_or(0, |p| p.total);
    let more = |route: &str, total: i64| -> Result<Option<String>, Error> {
        (total > SEARCH_PREVIEW_COUNT as i64)
            .then(|| url(route).map(|path| with_query(&path, &[("q", q.as_str())])))
            .transpose()
    };

    let tabs = Tab::ALL
        .into_iter()
        .map(|t| TabLink {
            tab: t.as_str(),
            label: match t {
                Tab::All => "Tất cả",
                Tab::Services => "Dịch vụ",
                Tab::Products => "Sản phẩm",
            },
            href: with_query(
                &search_url,
                &[
                    ("q", q.as_str()),
                    ("tab", if t == Tab::All { "" } else { t.as_str() }),
                ],
            ),
            count: match t {
                Tab::All => None,
                Tab::Services => Some(provider_total),
                Tab::Products => Some(product_total),
            },
            active: t == tab,
        })
        .collect();

    let page = SearchPage {
        providers_more: more("services", provider_total)?,
        products_more: more("products", product_total)?,
        q,
        tab: tab.as_str(),
        tabs,
        providers: providers
            .map(|p| {
                p.items
                    .into_iter()
                    .take(SEARCH_PREVIEW_COUNT)
                    .map(ProviderCard::from)
                    .collect()
            })
            .unwrap_or_default(),
        provider_total,
        products: products
            .map(|p| p.items.into_iter().take(SEARCH_PREVIEW_COUNT).collect())
            .unwrap_or_default(),
        product_total,
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
        assert_eq!(Tab::parse("ads"), Tab::All);
        assert_eq!(Tab::parse(""), Tab::All);
    }
}
