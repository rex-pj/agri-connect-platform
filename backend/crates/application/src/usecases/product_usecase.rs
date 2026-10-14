use std::sync::Arc;

use domain::{
    models::product::{ProductFilter, ProductView},
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
        sort: Option<String>,
        page: i64,
    ) -> Result<ProductBrowseResponse> {
        let (page, offset) = paginate(page, PAGE_SIZE);

        let (products, total) = tokio::try_join!(
            self.product_repo.browse(&filter, sort, PAGE_SIZE, offset),
            self.product_repo.count_browse(&filter),
        )?;

        Ok(ProductBrowseResponse {
            items: products.into_iter().map(map_product_dto).collect(),
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

        Ok(map_product_dto(product))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn map_product_dto(p: ProductView) -> ProductDto {
    ProductDto {
        id: p.id, name: p.name, description: p.description,
        price: p.price, unit: p.unit,
        category: p.category, vendor_name: p.vendor_name,
        vendor_phone: p.vendor_phone, is_available: p.is_available,
        created_on: p.created_on,
    }
}
