use std::sync::Arc;

use domain::{
    models::product::{ProductFilter, ProductSort},
    repositories::product_repository_trait::ProductRepositoryTrait,
    Error, ErrorCode, Result,
};

use crate::{
    constants::PAGE_SIZE,
    outputs::{ProductBrowseResponse, ProductDto},
    pagination::{paginate, total_pages},
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
        let (page, offset) = paginate(page, PAGE_SIZE);

        let (products, total) = tokio::try_join!(
            self.product_repo.browse(&filter, sort, PAGE_SIZE, offset),
            self.product_repo.count_browse(&filter),
        )?;

        Ok(ProductBrowseResponse {
            items: products.into_iter().map(ProductDto::from).collect(),
            total,
            page,
            per_page: PAGE_SIZE,
            total_pages: total_pages(total, PAGE_SIZE),
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
