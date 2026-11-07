//! GET /providers — the provider directory: every live business, searchable
//! (its name, description and its services' text) and filterable by the
//! services it offers. Providers with no service yet are listed too, so a
//! new sign-up can be found. The services themselves are `/services`.

use crate::{middleware::client_ip::ClientIp, pages::visitor};
use axum::{extract::State, response::Response, Extension};
use domain::{constants::CATEGORY_TYPE_SERVICE, Error};
use serde::Serialize;

use crate::{
    app_state::AppState,
    extract::ApiQuery,
    pages::{
        cards::ProviderCard,
        listing::{past_the_end, FilterForm, Pagination},
        search::TabLink,
        services::{
            borrowed, near_me_href, near_me_sub, url, view_tabs, ListQuery, ListState,
            VIEW_PROVIDERS,
        },
    },
    shell::Shell,
    views::render_page,
};

#[derive(Serialize)]
pub struct DirectoryPage {
    pub filters: FilterForm,
    pub providers: Vec<ProviderCard>,
    pub total: i64,
    pub pagination: Option<Pagination>,
    pub filtered: bool,
    pub back_href: String,
    /// Services or shops: the same search as either list.
    pub tabs: Vec<TabLink>,
    /// The near-me banner: its second line and link.
    pub geo_sub: String,
    pub near_href: String,
}

/// GET /providers
pub async fn list(
    shell: Shell,
    ip: Option<Extension<ClientIp>>,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let explore = &state.usecases.explore;
    let categories = explore.get_categories(Some(CATEGORY_TYPE_SERVICE)).await?;
    let list = ListState::parse(&q, &categories);
    let result = explore.browse_providers(list.params()).await?;
    if let Some(search) = list.search(result.total, visitor(&shell, ip.as_ref().map(|e| &e.0))) {
        state.usecases.demand.record_search(search);
    }

    let path = url("providers", &[])?;
    let kept = list.kept();
    if let Some(redirect) = past_the_end(&path, &borrowed(&kept), result.page, result.total_pages) {
        return redirect;
    }
    let page = DirectoryPage {
        pagination: Pagination::new(&path, &borrowed(&kept), result.page, result.total_pages),
        filtered: list.filtered(),
        filters: list.form(
            state.locale(),
            &path,
            &categories,
            "directory.search_placeholder",
        ),
        total: result.total,
        providers: result.items.into_iter().map(ProviderCard::from).collect(),
        back_href: url("home", &[])?,
        tabs: view_tabs(state.locale(), VIEW_PROVIDERS, &borrowed(&kept))?,
        geo_sub: near_me_sub(state.locale(), "geo.sub_shops"),
        near_href: near_me_href(VIEW_PROVIDERS, list.service_id)?,
    };
    render_page(&state, &shell, "pages/providers.html", &page)
}
