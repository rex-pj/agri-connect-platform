use async_trait::async_trait;

use crate::{error::Error, models::permission::PermissionModel};

/// Permissions are granted to roles only; a user's effective permissions are
/// those of their role.
#[async_trait]
pub trait PermissionRepositoryTrait {
    async fn list_all(&self) -> Result<Vec<PermissionModel>, Error>;

    async fn list_for_role(&self, role_id: i32) -> Result<Vec<PermissionModel>, Error>;

    /// Effective permissions of `user_id`, resolved through the user's role.
    async fn list_for_user(&self, user_id: i32) -> Result<Vec<PermissionModel>, Error>;

    /// Whether `user_id`'s role grants `key`. An inactive role grants nothing.
    async fn has_permission(&self, user_id: i32, key: &str) -> Result<bool, Error>;
}
