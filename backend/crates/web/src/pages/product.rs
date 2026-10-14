//! GET /products/{id} and /products/{id}/{slug} — product detail.
//!
//! One canonical URL per product, `/products/{id}/{slug of its name}`. Any
//! other spelling (no slug, an old slug after a rename, `007` for `7`) answers
//! 301 to it, so shared links keep working and search engines index one page.

use application::outputs::ProductDto;
use axum::{extract::State, response::Response};
use domain::{Error, ErrorCode};
use serde::Serialize;

use crate::{
    app_state::AppState,
    constants::META_DESCRIPTION_MAX_CHARS,
    extract::ApiPath,
    pages::{moved_permanently, summary},
    routes,
    shell::Shell,
    slug::slugify,
    views::render_page,
};

#[derive(Serialize)]
pub struct ProductPage {
    pub product: ProductDto,
    /// The description shortened for `<meta name="description">`; `None`
    /// when the product has none (the template writes a sentence instead).
    pub meta_description: Option<String>,
}

/// A path id that is not a number is a missing product, not a bad request:
/// a person typed or pasted it.
async fn load(state: &AppState, raw_id: &str) -> Result<ProductDto, Error> {
    let id = raw_id
        .parse::<i32>()
        .map_err(|_| Error::new(ErrorCode::ProductNotFound))?;
    state.usecases.product.get_product_by_id(id).await
}

fn canonical(product: &ProductDto) -> Result<String, Error> {
    let id = product.id.to_string();
    routes::url("product", &[("id", &id), ("slug", &slugify(&product.name))])
        .map_err(|e| Error::internal(format!("product url: {e}")))
}

/// GET /products/{id}
pub async fn by_id(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> Result<Response, Error> {
    let product = load(&state, &id).await?;
    moved_permanently(&canonical(&product)?)
}

/// GET /products/{id}/{slug}
pub async fn show(
    shell: Shell,
    State(state): State<AppState>,
    ApiPath((id, _slug)): ApiPath<(String, String)>,
) -> Result<Response, Error> {
    let product = load(&state, &id).await?;
    let canonical = canonical(&product)?;
    if shell.path != canonical {
        return moved_permanently(&canonical);
    }
    let page = ProductPage {
        meta_description: product
            .description
            .as_deref()
            .and_then(|d| summary(d, META_DESCRIPTION_MAX_CHARS)),
        product,
    };
    render_page(&state, &shell, "pages/product.html", &page)
}
