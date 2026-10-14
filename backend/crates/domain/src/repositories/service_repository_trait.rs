use async_trait::async_trait;

use crate::{
    error::Error,
    models::service::{AdminService, NewService, UpdateService},
};

#[async_trait]
pub trait ServiceRepositoryTrait {
    async fn list_admin(
        &self,
        provider_id: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AdminService>, Error>;

    async fn count_admin(&self, provider_id: Option<i32>) -> Result<i64, Error>;

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<AdminService>, Error>;

    async fn create(&self, service: NewService) -> Result<i32, Error>;

    async fn update(&self, service: UpdateService) -> Result<bool, Error>;

    async fn toggle_active(&self, id: i32, is_active: bool) -> Result<bool, Error>;

    async fn delete(&self, id: i32) -> Result<bool, Error>;

    async fn check_category_is_service(&self, category_id: i32) -> Result<bool, Error>;
}
