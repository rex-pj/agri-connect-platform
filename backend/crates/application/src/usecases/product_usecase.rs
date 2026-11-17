use std::sync::Arc;

use domain::{
    models::product::{ProductFilter, ProductSort},
    repositories::product_repository_trait::ProductRepositoryTrait,
    Error, ErrorCode, Result,
};

use crate::{
    constants::PAGE_SIZE,
    outputs::{ProductBrowseResponse, ProductDto},
    pagination::{paginate_public, public_total_pages, total_or_count},
    validation::search_text,
};

// ── Use case ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ProductUseCase {
    product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
}

impl ProductUseCase {
    pub fn new(product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>) -> Self {
        Self { product_repo }
    }

    pub async fn browse_products(
        &self,
        filter: ProductFilter,
        sort: ProductSort,
        page: i64,
    ) -> Result<ProductBrowseResponse> {
        let (page, offset) = paginate_public(page);
        let filter = ProductFilter {
            name: search_text(filter.name)?,
            ..filter
        };

        let products = self
            .product_repo
            .browse(&filter, sort, PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, products.len(), PAGE_SIZE, || {
            self.product_repo.count_browse(&filter)
        })
        .await?;

        Ok(ProductBrowseResponse {
            items: products.into_iter().map(ProductDto::from).collect(),
            total,
            page,
            per_page: PAGE_SIZE,
            total_pages: public_total_pages(total),
        })
    }

    pub async fn get_product_by_id(&self, id: i32) -> Result<ProductDto> {
        let product = self
            .product_repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ProductNotFound))?;

        Ok(ProductDto::from(product))
    }
}
