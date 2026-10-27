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
    pages::{
        canonical_url, moved_permanently, parse_id,
        service::{service_links, OtherService},
        summary,
    },
    shell::Shell,
    views::render_page,
};

#[derive(Serialize)]
pub struct ProductPage {
    pub product: ProductDto,
    /// The description shortened for `<meta name="description">`; `None`
    /// when the product has none (the template writes a sentence instead).
    pub meta_description: Option<String>,
    /// The seller's page, with its services and the contact form.
    pub provider_href: String,
    /// What else the seller does: their services, each to its own page.
    pub provider_services: Vec<OtherService>,
}

async fn load(state: &AppState, raw_id: &str) -> Result<ProductDto, Error> {
    let id = parse_id(raw_id, ErrorCode::ProductNotFound)?;
    state.usecases.product.get_product_by_id(id).await
}

fn canonical(product: &ProductDto) -> Result<String, Error> {
    canonical_url("product", product.id, &product.name)
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
    let provider = state
        .usecases
        .explore
        .get_provider_detail(product.business_id)
        .await?;
    let page = ProductPage {
        provider_services: service_links(&provider.services, None)?,
        provider_href: canonical_url("provider", product.business_id, &product.vendor_name)?,
        meta_description: product
            .description
            .as_deref()
            .and_then(|d| summary(d, META_DESCRIPTION_MAX_CHARS)),
        product,
    };
    render_page(&state, &shell, "pages/product.html", &page)
}
