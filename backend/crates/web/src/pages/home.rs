//! GET / — the home page.

use application::outputs::ProductDto;
use axum::{extract::State, response::Response};
use domain::Error;
use serde::Serialize;

use crate::{
    app_state::AppState,
    constants::{HOME_PRODUCT_COUNT, HOME_PROVIDER_COUNT},
    pages::cards::{tile, CategoryTile, ProviderCard},
    routes,
    shell::Shell,
    views::render_page,
};

/// The product category type in `categories.type`.
const PRODUCT_CATEGORY_TYPE: &str = "product";

#[derive(Serialize)]
pub struct HomePage {
    pub categories: Vec<CategoryTile>,
    pub products: Vec<ProductDto>,
    pub providers: Vec<ProviderCard>,
}

pub async fn show(shell: Shell, State(state): State<AppState>) -> Result<Response, Error> {
    let explore = &state.usecases.explore;
    let (featured, categories) = tokio::try_join!(
        explore.get_explore_data(),
        explore.get_categories(Some(PRODUCT_CATEGORY_TYPE)),
    )?;

    let url = |name: &str, params: &[(&str, &str)]| {
        routes::url(name, params).map_err(|e| Error::internal(format!("home links: {e}")))
    };
    let mut tiles = Vec::with_capacity(categories.len() + 2);
    for c in categories {
        let id = c.id.to_string();
        let href = url("product_category", &[("id", &id), ("slug", &c.slug)])?;
        tiles.push(tile(&c.name, c.icon.as_deref().unwrap_or("fa-tag"), href));
    }
    // Two shortcuts after the product categories, as on the SvelteKit home.
    tiles.push(tile("Dịch vụ", "fa-border-all", url("services", &[])?));
    tiles.push(tile("Gần tôi", "fa-location-dot", url("explore", &[])?));

    let page = HomePage {
        categories: tiles,
        products: featured
            .featured_products
            .into_iter()
            .take(HOME_PRODUCT_COUNT)
            .collect(),
        providers: featured
            .featured_providers
            .into_iter()
            .take(HOME_PROVIDER_COUNT)
            .map(ProviderCard::from)
            .collect(),
    };
    render_page(&state, &shell, "pages/home.html", &page)
}
