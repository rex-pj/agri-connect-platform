use std::{collections::BTreeSet, sync::Arc};

use domain::{
    events::{Change, DomainEvent, Entity},
    repositories::{
        permission_repository_trait::PermissionRepositoryTrait,
        role_repository_trait::{RoleGrantsRepositoryTrait, RoleRepositoryTrait},
        user_repository_trait::UserRepositoryTrait,
    },
    Error, ErrorCode, Result,
};

use crate::{
    authz::Actor,
    catalogue::{PERM_ROLE_PERMISSION_MANAGE, PERM_ROLE_READ, PERM_USER_READ},
    outputs::{
        GrantDto, PermissionDto, PermissionMatrixDto, PermissionMatrixRowDto, RoleDto,
        RoleGrantsDto, UserPermissionsDto,
    },
    ports::EventBus,
    validation::audit_reason,
};

/// The permission model: what exists, which role grants what, and editing a
/// role's grants. Permissions are granted to roles only.
#[derive(Clone)]
pub struct PermissionUseCase {
    /// On the primary pool: the role page reads its own writes right after
    /// a save, which a lagging replica could get wrong.
    permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    role_repo: Arc<dyn RoleRepositoryTrait + Send + Sync>,
    grants_repo: Arc<dyn RoleGrantsRepositoryTrait + Send + Sync>,
    events: Arc<dyn EventBus>,
}

/// Everything `PermissionUseCase` depends on, named so wiring cannot swap two
/// of compatible types by position.
pub struct PermissionDeps {
    pub permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
    pub user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    pub role_repo: Arc<dyn RoleRepositoryTrait + Send + Sync>,
    pub grants_repo: Arc<dyn RoleGrantsRepositoryTrait + Send + Sync>,
    pub events: Arc<dyn EventBus>,
}

impl PermissionUseCase {
    pub fn new(d: PermissionDeps) -> Self {
        Self {
            permission_repo: d.permission_repo,
            user_repo: d.user_repo,
            role_repo: d.role_repo,
            grants_repo: d.grants_repo,
            events: d.events,
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

    /// Every permission, each marked with whether `role_id` grants it.
    pub async fn role_grants(&self, actor: &Actor, role_id: i32) -> Result<RoleGrantsDto> {
        actor.require(PERM_ROLE_READ)?;
        let role = self
            .role_repo
            .find_by_id(role_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::RoleNotFound))?;
        let (all, granted) = tokio::try_join!(
            self.permission_repo.list_all(),
            self.permission_repo.list_for_role(role_id),
        )?;
        let granted: BTreeSet<String> = granted.into_iter().map(|p| p.code).collect();
        Ok(RoleGrantsDto {
            role: RoleDto::from(role),
            permissions: all
                .into_iter()
                .map(|p| GrantDto {
                    granted: granted.contains(&p.code),
                    code: p.code,
                    name: p.name,
                })
                .collect(),
        })
    }

    /// Permissions × roles, for the overview page.
    pub async fn matrix(&self, actor: &Actor) -> Result<PermissionMatrixDto> {
        actor.require(PERM_ROLE_READ)?;
        let (all, roles) =
            tokio::try_join!(self.permission_repo.list_all(), self.role_repo.list())?;
        let mut grants = Vec::with_capacity(roles.len());
        for role in &roles {
            let codes: BTreeSet<String> = self
                .permission_repo
                .list_for_role(role.id)
                .await?
                .into_iter()
                .map(|p| p.code)
                .collect();
            grants.push(codes);
        }
        Ok(PermissionMatrixDto {
            rows: all
                .into_iter()
                .map(|p| PermissionMatrixRowDto {
                    granted: grants.iter().map(|g| g.contains(&p.code)).collect(),
                    code: p.code,
                    name: p.name,
                })
                .collect(),
            roles: roles.into_iter().map(RoleDto::from).collect(),
        })
    }

    /// Makes `role_id` grant exactly `keys`. Takes effect on the next request
    /// of every user of that role (permissions are read per request).
    ///
    /// Refuses to take `role.permission.manage` away from the caller's own
    /// role: nobody would be left who could give it back.
    pub async fn set_role_grants(
        &self,
        actor: &Actor,
        role_id: i32,
        keys: Vec<String>,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_ROLE_PERMISSION_MANAGE)?;
        let role = self
            .role_repo
            .find_by_id(role_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::RoleNotFound))?;
        let known: BTreeSet<String> = self
            .permission_repo
            .list_all()
            .await?
            .into_iter()
            .map(|p| p.code)
            .collect();
        let keys: BTreeSet<String> = keys.into_iter().collect();
        if let Some(unknown) = keys.iter().find(|k| !known.contains(*k)) {
            return Err(Error::new(ErrorCode::UnknownPermission).with_param("key", unknown));
        }
        // The caller's own role is identified by its code, which is what the
        // session carries; this is about who would be locked out, not access.
        if role.code == actor.role_code && !keys.contains(PERM_ROLE_PERMISSION_MANAGE) {
            return Err(Error::new(ErrorCode::CannotRevokeOwnAccess));
        }

        let keys: Vec<String> = keys.into_iter().collect();
        let changes = self
            .grants_repo
            .replace(role_id, &keys, actor.user_id)
            .await?;
        if changes.added.is_empty() && changes.removed.is_empty() {
            return Ok(());
        }
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity: Entity::Role,
                entity_id: role_id,
                change: Change::PermissionsChanged {
                    added: changes.added,
                    removed: changes.removed,
                },
                reason: audit_reason(reason),
            })
            .await;
        Ok(())
    }
}
