use async_trait::async_trait;

use crate::{
    models::category::{Category, CategoryAdmin, NewCategory, UpdateCategory},
    error::Error,
};

#[async_trait]
pub trait CategoryRepositoryTrait {
    async fn list(&self, type_filter: Option<&str>) -> Result<Vec<Category>, Error>;

    async fn list_admin(&self) -> Result<Vec<CategoryAdmin>, Error>;

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<CategoryAdmin>, Error>;

    async fn create(&self, category: NewCategory) -> Result<i32, Error>;

    async fn update(&self, id: i32, category: UpdateCategory) -> Result<(), Error>;

    async fn toggle_active(&self, id: i32, is_active: bool, admin_id: i32) -> Result<(), Error>;
}
