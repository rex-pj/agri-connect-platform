//! GET / — the home page: one strip of category shortcuts (services first,
//! the farmer's main errand, then products, told apart by tile shape and
//! colour), the near-me banner, featured services (one per verified
//! provider) and featured products.

use application::outputs::{CategoryDto, ProductDto};
use axum::{extract::State, response::Response};
use domain::{
    constants::{CATEGORY_TYPE_PRODUCT, CATEGORY_TYPE_SERVICE},
    Error,
};
use serde::Serialize;

use crate::{
    app_state::AppState,
    constants::{HOME_CATEGORY_TILES, HOME_PRODUCT_COUNT},
    pages::cards::{rated_services, tile, CategoryTile, ServiceCard},
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

/// The first `HOME_CATEGORY_TILES` categories of `kind`, each linking to its
/// listing (`route`), then an "all" tile to the kind's whole list
/// (`all_route`). A phone row holds one tile fewer, so the last category is
/// wide-only: either way a kind fills exactly one row. No categories, no
/// tiles.
fn category_tiles(
    state: &AppState,
    categories: &[CategoryDto],
    kind: &'static str,
    route: &str,
    all_route: &str,
) -> Result<Vec<CategoryTile>, Error> {
    if categories.is_empty() {
        return Ok(Vec::new());
    }
    let mut tiles = categories
        .iter()
        .take(HOME_CATEGORY_TILES)
        .enumerate()
        .map(|(i, c)| {
            let href = url(route, &[("id", &c.id.to_string()), ("slug", &c.slug)])?;
            Ok(CategoryTile {
                kind,
                wide_only: i + 1 == HOME_CATEGORY_TILES,
                ..tile(&c.name, c.icon.as_deref().unwrap_or("fa-tag"), href)
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    tiles.push(CategoryTile {
        kind,
        more: true,
        ..tile(
            &state.locale().text("home.all_categories"),
            "fa-grip",
            url(all_route, &[])?,
        )
    });
    Ok(tiles)
}

pub async fn show(shell: Shell, State(state): State<AppState>) -> Result<Response, Error> {
    let explore = &state.usecases.explore;
    let (featured, service_categories, product_categories) = tokio::try_join!(
        explore.get_explore_data(),
        explore.get_categories(Some(CATEGORY_TYPE_SERVICE)),
        explore.get_categories(Some(CATEGORY_TYPE_PRODUCT)),
    )?;

    // Near-me needs no tile: it has a tab and the banner under the strip.
    let mut tiles = category_tiles(
        &state,
        &service_categories,
        CATEGORY_TYPE_SERVICE,
        "service_category",
        "services",
    )?;
    tiles.extend(category_tiles(
        &state,
        &product_categories,
        CATEGORY_TYPE_PRODUCT,
        "product_category",
        "products",
    )?);

    let page = HomePage {
        categories: tiles,
        services: rated_services(&state, featured.featured_services).await?,
        products: featured
            .featured_products
            .into_iter()
            .take(HOME_PRODUCT_COUNT)
            .collect(),
    };
    render_page(&state, &shell, "pages/home.html", &page)
}
