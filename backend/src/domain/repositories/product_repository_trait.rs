use async_trait::async_trait;

use crate::{
    domain::models::product::{NewProduct, ProductView, ProviderProduct, UpdateProduct},
    view_models::AppError,
};

#[async_trait]
pub trait ProductRepositoryTrait {
    async fn browse(
        &self,
        category_id: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ProductView>, AppError>;

    async fn count_browse(&self, category_id: Option<i32>) -> Result<i64, AppError>;

    async fn featured(&self, limit: i64) -> Result<Vec<ProductView>, AppError>;

    async fn find_by_id(&self, id: i32) -> Result<Option<ProductView>, AppError>;

    async fn list_by_user(&self, user_id: i32) -> Result<Vec<ProviderProduct>, AppError>;

    async fn check_category_is_product(&self, category_id: i32) -> Result<bool, AppError>;

    async fn create(&self, product: NewProduct) -> Result<i32, AppError>;

    async fn find_provider_product_by_id(&self, id: i32) -> Result<Option<ProviderProduct>, AppError>;

    async fn update(&self, product: UpdateProduct) -> Result<bool, AppError>;

    async fn soft_delete(&self, id: i32, owner_user: i32) -> Result<bool, AppError>;

    async fn count_active(&self) -> Result<i64, AppError>;
}
