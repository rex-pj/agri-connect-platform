//! Former addresses, moved for good (301) with their query: bookmarks,
//! shared links and old emails still land on the page. The admin's provider
//! list was `/admin/explore`; supplies ("vật tư")
//! were "products" at `/products`, `/admin/products`, `/provider/products`;
//! a product's or a category's own page needs nothing here, its handler
//! sends any other spelling to its canonical address.

use axum::{extract::RawQuery, response::Response};
use domain::Error;

use crate::{extract::ApiPath, pages::moved_permanently, routes};

/// 301 to the route `name` with `params`, keeping the query.
fn to(name: &str, params: &[(&str, &str)], query: Option<String>) -> Result<Response, Error> {
    let path =
        routes::url(name, params).map_err(|e| Error::internal(format!("moved address: {e}")))?;
    match query.filter(|q| !q.is_empty()) {
        Some(q) => moved_permanently(&format!("{path}?{q}")),
        None => moved_permanently(&path),
    }
}

/// GET /products → /supplies
pub async fn supplies(RawQuery(query): RawQuery) -> Result<Response, Error> {
    to("products", &[], query)
}

/// GET /admin/products → /admin/supplies
pub async fn admin_supplies(RawQuery(query): RawQuery) -> Result<Response, Error> {
    to("admin_products", &[], query)
}

/// GET /admin/products/{id} → /admin/supplies/{id}
pub async fn admin_supply(ApiPath(id): ApiPath<String>) -> Result<Response, Error> {
    to("admin_product", &[("id", &id)], None)
}

/// GET /provider/products/new → /provider/supplies/new
pub async fn own_supply_new() -> Result<Response, Error> {
    to("provider_product_new", &[], None)
}

/// GET /provider/products/{id}/edit → /provider/supplies/{id}/edit
pub async fn own_supply_edit(ApiPath(id): ApiPath<String>) -> Result<Response, Error> {
    to("provider_product_edit", &[("id", &id)], None)
}

/// GET /admin/explore → /admin/providers (the provider list's former name)
pub async fn admin_providers(RawQuery(query): RawQuery) -> Result<Response, Error> {
    to("admin_providers", &[], query)
}

/// GET /admin/explore/{id} → /admin/providers/{id}
pub async fn admin_provider(ApiPath(id): ApiPath<String>) -> Result<Response, Error> {
    to("admin_provider", &[("id", &id)], None)
}
