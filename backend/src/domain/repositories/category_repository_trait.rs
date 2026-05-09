use async_trait::async_trait;

use crate::{
    domain::models::category::{Category, CategoryAdmin, NewCategory, UpdateCategory},
    view_models::AppError,
};

#[async_trait]
pub trait CategoryRepositoryTrait {
    async fn list(&self, type_filter: Option<&str>) -> Result<Vec<Category>, AppError>;

    async fn count_active(&self) -> Result<i64, AppError>;

    async fn list_admin(&self) -> Result<Vec<CategoryAdmin>, AppError>;

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<CategoryAdmin>, AppError>;

    async fn create(&self, category: NewCategory) -> Result<i32, AppError>;

    async fn update(&self, id: i32, category: UpdateCategory) -> Result<(), AppError>;

    async fn toggle_active(&self, id: i32, is_active: bool) -> Result<(), AppError>;
}
