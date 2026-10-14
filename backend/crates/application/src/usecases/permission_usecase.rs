use std::sync::Arc;

use domain::{
    repositories::{
        permission_repository_trait::PermissionRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Error, ErrorCode, Result,
};

use crate::{
    authz::Actor,
    catalogue::{PERM_ROLE_READ, PERM_USER_READ},
    outputs::{PermissionDto, UserPermissionsDto},
};

/// Read-only views of the permission model. Grants belong to roles; editing
/// them arrives with the role permission grid.
#[derive(Clone)]
pub struct PermissionUseCase {
    permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
}

impl PermissionUseCase {
    pub fn new(
        permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
        user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    ) -> Self {
        Self {
            permission_repo,
            user_repo,
        }
    }

    pub async fn list_all(&self, actor: &Actor) -> Result<Vec<PermissionDto>> {
        actor.require(PERM_ROLE_READ)?;
        let perms = self.permission_repo.list_all().await?;
        Ok(perms.into_iter().map(PermissionDto::from_model).collect())
    }

    /// The user's effective permissions, i.e. those of their role.
    pub async fn list_by_user(&self, actor: &Actor, user_id: i32) -> Result<UserPermissionsDto> {
        actor.require(PERM_USER_READ)?;
        if self
            .user_repo
            .find_by_id_for_admin(user_id)
            .await?
            .is_none()
        {
            return Err(Error::new(ErrorCode::UserNotFound));
        }
        let perms = self.permission_repo.list_for_user(user_id).await?;
        Ok(UserPermissionsDto {
            user_id,
            permissions: perms.into_iter().map(PermissionDto::from_model).collect(),
        })
    }
}
