use async_trait::async_trait;

use crate::{
    error::Error,
    models::category::{Category, CategoryAdmin, NewCategory, UpdateCategory},
};

#[async_trait]
pub trait CategoryRepositoryTrait {
    async fn list(&self, type_filter: Option<&str>) -> Result<Vec<Category>, Error>;

    async fn list_admin(&self) -> Result<Vec<CategoryAdmin>, Error>;

    /// The ids of the types of `category_type` that some live service or
    /// supply is filed under.
    async fn used_ids(&self, category_type: &str) -> Result<Vec<i32>, Error>;

    /// A page of one type's categories (`CATEGORY_TYPE_SERVICE` or
    /// `CATEGORY_TYPE_PRODUCT`), hidden ones too, by display order then id.
    async fn list_admin_page(
        &self,
        category_type: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CategoryAdmin>, Error>;

    /// How many categories of that type there are, hidden ones too.
    async fn count_admin(&self, category_type: &str) -> Result<i64, Error>;

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<CategoryAdmin>, Error>;

    async fn create(&self, category: NewCategory) -> Result<i32, Error>;

    async fn update(&self, id: i32, category: UpdateCategory) -> Result<(), Error>;

    /// Which of `slugs` a category other than `exclude_id` already uses.
    async fn taken_slugs(
        &self,
        slugs: &[String],
        exclude_id: Option<i32>,
    ) -> Result<Vec<String>, Error>;

    async fn toggle_active(&self, id: i32, is_active: bool, admin_id: i32) -> Result<(), Error>;
}
