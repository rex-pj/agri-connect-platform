use std::sync::Arc;

use crate::{
    domain::repositories::role_repository_trait::RoleRepositoryTrait,
    view_models::{responses::RoleDto, AppError},
};

#[derive(Clone)]
pub struct RoleUseCase {
    role_repo: Arc<dyn RoleRepositoryTrait + Send + Sync>,
}

impl RoleUseCase {
    pub fn new(role_repo: Arc<dyn RoleRepositoryTrait + Send + Sync>) -> Self {
        Self { role_repo }
    }

    pub async fn list(&self) -> Result<Vec<RoleDto>, AppError> {
        self.role_repo.list().await.map(|roles| {
            roles
                .into_iter()
                .map(|r| RoleDto {
                    id: r.id, code: r.code, name: r.name,
                    description: r.description, is_active: r.is_active,
                })
                .collect()
        })
    }

    pub async fn get_by_id(&self, id: i32) -> Result<RoleDto, AppError> {
        self.role_repo
            .find_by_id(id)
            .await?
            .map(|r| RoleDto {
                id: r.id, code: r.code, name: r.name,
                description: r.description, is_active: r.is_active,
            })
            .ok_or_else(|| AppError::NotFound("Không tìm thấy vai trò.".into()))
    }
}
