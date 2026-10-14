use async_trait::async_trait;

use crate::{
    models::product::{AdminProduct, NewProduct, ProductFilter, ProductView, ProviderProduct, UpdateProduct},
    error::Error,
};

#[async_trait]
pub trait ProductRepositoryTrait {
    async fn list_admin(&self, provider_id: Option<i32>, limit: i64, offset: i64) -> Result<Vec<AdminProduct>, Error>;

    async fn count_admin(&self, provider_id: Option<i32>) -> Result<i64, Error>;

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<AdminProduct>, Error>;

    async fn admin_toggle_active(&self, id: i32, is_active: bool) -> Result<bool, Error>;

    async fn admin_delete(&self, id: i32) -> Result<bool, Error>;

    async fn browse(
        &self,
        filter: &ProductFilter,
        sort: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ProductView>, Error>;

    async fn count_browse(&self, filter: &ProductFilter) -> Result<i64, Error>;

    async fn featured(&self, limit: i64) -> Result<Vec<ProductView>, Error>;

    async fn find_by_id(&self, id: i32) -> Result<Option<ProductView>, Error>;

    async fn list_by_user(&self, user_id: i32) -> Result<Vec<ProviderProduct>, Error>;

    async fn check_category_is_product(&self, category_id: i32) -> Result<bool, Error>;

    async fn create(&self, product: NewProduct) -> Result<i32, Error>;

    async fn find_provider_product_by_id(&self, id: i32) -> Result<Option<ProviderProduct>, Error>;

    async fn update(&self, product: UpdateProduct) -> Result<bool, Error>;

    async fn soft_delete(&self, id: i32, owner_user: i32) -> Result<bool, Error>;

    async fn count_active(&self) -> Result<i64, Error>;
}
