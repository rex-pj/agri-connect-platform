use async_trait::async_trait;

use crate::{domain::models::permission::PermissionModel, view_models::AppError};

#[async_trait]
pub trait PermissionRepositoryTrait {
    async fn list_all(&self) -> Result<Vec<PermissionModel>, AppError>;

    async fn find_by_id(&self, id: i32) -> Result<Option<PermissionModel>, AppError>;

    async fn list_by_user(&self, user_id: i32) -> Result<Vec<PermissionModel>, AppError>;

    /// Returns `true` if `user_id` has been granted the permission identified
    /// by `code`.  Admin bypass is handled at the use-case level — this method
    /// performs only the raw DB check.
    async fn has_permission(&self, user_id: i32, code: &str) -> Result<bool, AppError>;

    async fn assign_to_user(
        &self,
        user_id:      i32,
        permission_id: i32,
        granted_by:   i32,
    ) -> Result<(), AppError>;

    /// Assigns every permission in `codes` to `user_id` in one SQL statement,
    /// silently skipping codes that are already granted or do not exist in the
    /// `permissions` table.  Used when a new provider registers.
    async fn assign_defaults_to_user(
        &self,
        user_id: i32,
        codes:   &[&str],
    ) -> Result<(), AppError>;

    async fn revoke_from_user(
        &self,
        user_id:      i32,
        permission_id: i32,
    ) -> Result<(), AppError>;
}
