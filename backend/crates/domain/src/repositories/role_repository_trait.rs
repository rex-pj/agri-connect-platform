use async_trait::async_trait;

use crate::{error::Error, models::role::RoleModel};

#[async_trait]
pub trait RoleRepositoryTrait {
    async fn list(&self) -> Result<Vec<RoleModel>, Error>;

    async fn find_by_id(&self, id: i32) -> Result<Option<RoleModel>, Error>;

    async fn find_active_by_code(&self, code: &str) -> Result<Option<RoleModel>, Error>;
}
