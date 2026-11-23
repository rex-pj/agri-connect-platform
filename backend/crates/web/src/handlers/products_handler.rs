use axum::{extract::State, response::IntoResponse, Json};
use domain::{
    models::product::{ProductFilter, ProductSort},
    Error,
};

use crate::{
    app_state::AppState,
    extract::{ApiPath, ApiQuery},
    requests::ProductBrowseQuery,
};

pub struct ProductsHandler;

impl ProductsHandler {
    /// GET /api/explore/products?category_id=&name=&page=
    pub async fn browse_products(
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<ProductBrowseQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state
            .usecases
            .product
            .browse_products(
                ProductFilter {
                    category_id: q.category_id,
                    commodity_id: q.commodity_id,
                    name: q.name,
                    min_price: q.min_price,
                    max_price: q.max_price,
                    ..ProductFilter::default()
                },
                ProductSort::parse(q.sort.as_deref()),
                q.page.unwrap_or(1),
            )
            .await?;

        Ok(Json(result))
    }

    /// GET /api/explore/products/:id
    pub async fn get_product_by_id(
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
    ) -> Result<impl IntoResponse, Error> {
        let result = state.usecases.product.get_product_by_id(id).await?;

        Ok(Json(result))
    }
}
