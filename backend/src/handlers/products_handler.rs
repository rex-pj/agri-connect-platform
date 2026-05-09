use axum::{
    extract::{Path, Query, State},
    Json,
};

use crate::{
    app_state::AppState,
    view_models::{
        requests::ProductBrowseQuery,
        HandlerError, HandlerResult,
    },
};

pub struct ProductsHandler;

impl ProductsHandler {
    /// GET /api/agri-connect/products/browse?category=&page=
    pub async fn browse_products(
        State(state): State<AppState>,
        Query(q): Query<ProductBrowseQuery>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .product
            .browse_products(q.category_id, q.page.unwrap_or(1))
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }

    /// GET /api/agri-connect/products/:id
    pub async fn get_product_by_id(
        State(state): State<AppState>,
        Path(id): Path<i32>,
    ) -> HandlerResult<impl axum::response::IntoResponse> {
        let result = state
            .usecases
            .product
            .get_product_by_id(id)
            .await
            .map_err(HandlerError::from)?;

        Ok(Json(result))
    }
}
