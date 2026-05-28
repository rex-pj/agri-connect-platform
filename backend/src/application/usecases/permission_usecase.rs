use std::sync::Arc;

use crate::{
    domain::{
        repositories::{
            permission_repository_trait::PermissionRepositoryTrait,
            user_repository_trait::UserRepositoryTrait,
        },
    },
    view_models::{
        responses::{PermissionDto, UserPermissionsDto},
        AppError,
    },
};

#[derive(Clone)]
pub struct PermissionUseCase {
    permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
    user_repo:       Arc<dyn UserRepositoryTrait       + Send + Sync>,
}

impl PermissionUseCase {
    pub fn new(
        permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
        user_repo:       Arc<dyn UserRepositoryTrait       + Send + Sync>,
    ) -> Self {
        Self { permission_repo, user_repo }
    }

    pub async fn list_all(&self) -> Result<Vec<PermissionDto>, AppError> {
        let perms = self.permission_repo.list_all().await?;
        Ok(perms.into_iter().map(PermissionDto::from_model).collect())
    }

    pub async fn list_by_user(&self, user_id: i32) -> Result<UserPermissionsDto, AppError> {
        if self.user_repo.find_by_id_for_admin(user_id).await?.is_none() {
            return Err(AppError::NotFound("User not found.".into()));
        }
        let perms = self.permission_repo.list_by_user(user_id).await?;
        Ok(UserPermissionsDto {
            user_id,
            permissions: perms.into_iter().map(PermissionDto::from_model).collect(),
        })
    }

    pub async fn assign(
        &self,
        user_id:       i32,
        permission_id: i32,
        granted_by:    i32,
    ) -> Result<(), AppError> {
        if self.user_repo.find_by_id_for_admin(user_id).await?.is_none() {
            return Err(AppError::NotFound("User not found.".into()));
        }
        if self.permission_repo.find_by_id(permission_id).await?.is_none() {
            return Err(AppError::NotFound("Permission not found.".into()));
        }
        self.permission_repo.assign_to_user(user_id, permission_id, granted_by).await
    }

    pub async fn revoke(&self, user_id: i32, permission_id: i32) -> Result<(), AppError> {
        if self.user_repo.find_by_id_for_admin(user_id).await?.is_none() {
            return Err(AppError::NotFound("User not found.".into()));
        }
        self.permission_repo.revoke_from_user(user_id, permission_id).await
    }

}
