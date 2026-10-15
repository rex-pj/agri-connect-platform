use async_trait::async_trait;

use crate::{error::Error, models::role::RoleModel};

#[async_trait]
pub trait RoleRepositoryTrait {
    async fn list(&self) -> Result<Vec<RoleModel>, Error>;

    async fn find_by_id(&self, id: i32) -> Result<Option<RoleModel>, Error>;

    async fn find_active_by_code(&self, code: &str) -> Result<Option<RoleModel>, Error>;
}

/// What `RoleGrantsRepositoryTrait::replace` changed, by permission key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrantChanges {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

/// Writes to `role_permissions`. Separate from the permission reads, which
/// may run on a read replica; this one always runs on the primary.
#[async_trait]
pub trait RoleGrantsRepositoryTrait {
    /// Makes `role_id` grant exactly `keys` (unknown keys are ignored by the
    /// caller beforehand), recording `granted_by`, in one transaction.
    async fn replace(
        &self,
        role_id: i32,
        keys: &[String],
        granted_by: i32,
    ) -> Result<GrantChanges, Error>;
}
