//! GET / — the home page: category shortcuts (services first, the farmer's
//! main errand, then products), the near-me banner, featured services (one
//! per verified provider) and featured products.

use application::outputs::{CategoryDto, ProductDto};
use axum::{extract::State, response::Response};
use domain::{
    constants::{CATEGORY_TYPE_PRODUCT, CATEGORY_TYPE_SERVICE},
    Error,
};
use serde::Serialize;

use crate::{
    app_state::AppState,
    constants::HOME_PRODUCT_COUNT,
    pages::cards::{tile, CategoryTile, ServiceCard},
    routes,
    shell::Shell,
    views::render_page,
};

#[derive(Serialize)]
pub struct HomePage {
    pub categories: Vec<CategoryTile>,
    pub services: Vec<ServiceCard>,
    pub products: Vec<ProductDto>,
}

fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("home links: {e}")))
}

/// One tile per category, linking to its listing (`route`).
fn category_tiles(categories: &[CategoryDto], route: &str) -> Result<Vec<CategoryTile>, Error> {
    categories
        .iter()
        .map(|c| {
            let href = url(route, &[("id", &c.id.to_string()), ("slug", &c.slug)])?;
            Ok(tile(&c.name, c.icon.as_deref().unwrap_or("fa-tag"), href))
        })
        .collect()
}

pub async fn show(shell: Shell, State(state): State<AppState>) -> Result<Response, Error> {
    let explore = &state.usecases.explore;
    let (featured, service_categories, product_categories) = tokio::try_join!(
        explore.get_explore_data(),
        explore.get_categories(Some(CATEGORY_TYPE_SERVICE)),
        explore.get_categories(Some(CATEGORY_TYPE_PRODUCT)),
    )?;

    let mut tiles = category_tiles(&service_categories, "service_category")?;
    tiles.extend(category_tiles(&product_categories, "product_category")?);
    tiles.push(tile(
        &state.locale().text("nav.near_me"),
        "fa-location-dot",
        url("explore", &[])?,
    ));
    // The provider directory has no tab on a phone's bar; it is here.
    tiles.push(tile(
        &state.locale().text("nav.shops"),
        "fa-store",
        url("providers", &[])?,
    ));

    let page = HomePage {
        categories: tiles,
        services: ServiceCard::all(featured.featured_services)?,
        products: featured
            .featured_products
            .into_iter()
            .take(HOME_PRODUCT_COUNT)
            .collect(),
    };
    render_page(&state, &shell, "pages/home.html", &page)
}
