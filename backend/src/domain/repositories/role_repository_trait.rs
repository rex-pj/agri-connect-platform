use async_trait::async_trait;

use crate::{
    domain::models::role::RoleModel,
    view_models::AppError,
};

#[async_trait]
pub trait RoleRepositoryTrait {
    async fn list(&self) -> Result<Vec<RoleModel>, AppError>;

    async fn find_by_id(&self, id: i32) -> Result<Option<RoleModel>, AppError>;
}
