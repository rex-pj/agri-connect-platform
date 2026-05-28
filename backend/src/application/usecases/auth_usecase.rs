use std::sync::Arc;

use chrono::{Duration, Utc};
use rand::{rngs::OsRng, Rng};

use crate::{
    application::{
        ports::{EmailService, PasswordHasher, TokenService},
        shared::{validate_password, validate_phone, validate_username},
    },
    constants::{
        LOGIN_FAIL_MAX_ATTEMPTS, LOGIN_LOCK_MINUTES, PASSWORD_RESET_EXPIRY_HOURS,
        PROVIDER_DEFAULT_PERMISSIONS, ROLE_ADMIN, ROLE_PROVIDER,
    },
    domain::{
        models::{
            role::UserRole,
            user::{NewResetToken, NewUser},
        },
        repositories::{
            business_repository_trait::BusinessRepositoryTrait,
            permission_repository_trait::PermissionRepositoryTrait,
            role_repository_trait::RoleRepositoryTrait,
            user_repository_trait::UserRepositoryTrait,
        },
    },
    view_models::{
        responses::{LoginResponse, MeResponse},
        AppError,
    },
};

// ── Use case ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AuthUseCase {
    user_repo:       Arc<dyn UserRepositoryTrait       + Send + Sync>,
    business_repo:   Arc<dyn BusinessRepositoryTrait   + Send + Sync>,
    role_repo:       Arc<dyn RoleRepositoryTrait        + Send + Sync>,
    permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
    /// Email sending is an infrastructure concern injected via port.
    email:           Arc<dyn EmailService>,
    /// Password hashing is an infrastructure concern injected via port.
    password_hasher: Arc<dyn PasswordHasher>,
    /// Token minting is an infrastructure concern injected via port.
    token_svc:       Arc<dyn TokenService>,
}

impl AuthUseCase {
    pub fn new(
        user_repo:       Arc<dyn UserRepositoryTrait       + Send + Sync>,
        business_repo:   Arc<dyn BusinessRepositoryTrait   + Send + Sync>,
        role_repo:       Arc<dyn RoleRepositoryTrait        + Send + Sync>,
        permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
        email:           Arc<dyn EmailService>,
        password_hasher: Arc<dyn PasswordHasher>,
        token_svc:       Arc<dyn TokenService>,
    ) -> Self {
        Self { user_repo, business_repo, role_repo, permission_repo, email, password_hasher, token_svc }
    }

    // ── Login ─────────────────────────────────────────────────────────────────

    pub async fn login(
        &self,
        username: String,
        password: String,
    ) -> Result<LoginResponse, AppError> {
        let username = username.trim().to_lowercase();

        let credentials = self
            .user_repo
            .find_credentials_by_username(&username)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Invalid username or password.".into()))?;

        // Explicit domain invariant: inactive accounts cannot log in.
        if !credentials.is_active {
            return Err(AppError::Forbidden(
                "This account has been deactivated. Please contact support.".into(),
            ));
        }

        if let Some(locked_until) = credentials.locked_until {
            if locked_until > Utc::now() {
                return Err(AppError::AccountLocked(
                    "Account temporarily locked due to too many failed login attempts. Please try again later.".into(),
                ));
            }
        }

        let ok = self.password_hasher.verify(&password, &credentials.password_hash).await?;

        if !ok {
            let _ = self
                .user_repo
                .record_failed_login(credentials.id, LOGIN_FAIL_MAX_ATTEMPTS, LOGIN_LOCK_MINUTES)
                .await;
            return Err(AppError::Unauthorized("Invalid username or password.".into()));
        }

        let _ = self.user_repo.reset_failed_login(credentials.id).await;

        let role = UserRole::from_code(&credentials.role)
            .ok_or_else(|| AppError::Forbidden("Account does not have access.".into()))?;

        let token = self.token_svc.mint(credentials.id, &username, &role)?;
        Ok(LoginResponse {
            token,
            username,
            role: credentials.role,
        })
    }

    // ── First-admin bootstrap ─────────────────────────────────────────────────

    pub async fn create_first_admin(
        &self,
        username: String,
        email: String,
        password: String,
        full_name: Option<String>,
    ) -> Result<LoginResponse, AppError> {
        let admin_count = self.user_repo.count_by_role(ROLE_ADMIN).await?;
        if admin_count > 0 {
            return Err(AppError::Forbidden(
                "An admin account already exists. Please log in.".into(),
            ));
        }

        self.seed_reference_data().await?;

        let username = username.trim().to_lowercase();
        validate_username(&username)?;
        validate_password(&password)?;

        let password_hash = self.password_hasher.hash(&password).await?;
        let email = email.trim().to_lowercase();
        let full_name = full_name.as_deref().unwrap_or("Administrator").to_owned();

        let user_id = self
            .user_repo
            .create(NewUser {
                username: username.clone(),
                email,
                password_hash,
                role: ROLE_ADMIN.into(),
                full_name: Some(full_name),
            })
            .await
            .map_err(|_| AppError::Internal("Failed to create admin account.".into()))?;

        let token = self.token_svc.mint(user_id, &username, &UserRole::Admin)?;
        Ok(LoginResponse {
            token,
            username,
            role: ROLE_ADMIN.into(),
        })
    }

    async fn seed_reference_data(&self) -> Result<(), AppError> {
        self.role_repo.seed_defaults().await
    }

    // ── Provider self-registration ────────────────────────────────────────────

    pub async fn register(
        &self,
        username: String,
        email: String,
        password: String,
        full_name: Option<String>,
        business_name: String,
        business_phone: String,
    ) -> Result<LoginResponse, AppError> {
        let username = username.trim().to_lowercase();
        validate_username(&username)?;
        validate_password(&password)?;

        let business_name = business_name.trim().to_owned();
        let business_phone = business_phone.trim().to_owned();
        if business_name.is_empty() {
            return Err(AppError::BadRequest("Business name must not be empty.".into()));
        }
        if business_phone.is_empty() {
            return Err(AppError::BadRequest("Business phone must not be empty.".into()));
        }
        validate_phone(&business_phone)?;

        let email = email.trim().to_lowercase();

        if self.user_repo.find_by_username(&username).await?.is_some() {
            return Err(AppError::Conflict("Username already exists.".into()));
        }
        if self.user_repo.email_exists(&email).await? {
            return Err(AppError::Conflict("Email is already in use.".into()));
        }

        let password_hash = self.password_hasher.hash(&password).await?;

        let user_id = self
            .user_repo
            .create(NewUser {
                username: username.clone(),
                email,
                password_hash,
                role: ROLE_PROVIDER.into(),
                full_name,
            })
            .await
            .map_err(|_| AppError::Internal("Failed to create account.".into()))?;

        self.business_repo
            .create_for_registration(&business_name, &business_phone, user_id)
            .await?;

        // Grant every default provider permission so the new account can manage
        // its profile and products immediately. Admins can revoke specific
        // permissions later if needed.
        self.permission_repo
            .assign_defaults_to_user(user_id, PROVIDER_DEFAULT_PERMISSIONS)
            .await?;

        let token = self.token_svc.mint(user_id, &username, &UserRole::Provider)?;
        Ok(LoginResponse {
            token,
            username,
            role: ROLE_PROVIDER.into(),
        })
    }

    // ── Me (authenticated) ────────────────────────────────────────────────────

    pub async fn me(&self, user_id: i32) -> Result<MeResponse, AppError> {
        let user = self
            .user_repo
            .find_me(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Account not found.".into()))?;

        Ok(MeResponse {
            username:   user.username,
            email:      user.email,
            full_name:  user.full_name,
            role:       user.role,
            created_on: user.created_on.to_rfc3339(),
        })
    }

    // ── Change password (authenticated) ──────────────────────────────────────

    pub async fn change_password(
        &self,
        user_id: i32,
        current_password: String,
        new_password: String,
    ) -> Result<(), AppError> {
        validate_password(&new_password)?;

        let credentials = self
            .user_repo
            .find_credentials_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Account not found.".into()))?;

        // Inactive accounts must not be allowed to change their password.
        if !credentials.is_active {
            return Err(AppError::Forbidden(
                "This account has been deactivated.".into(),
            ));
        }

        let ok = self.password_hasher.verify(&current_password, &credentials.password_hash).await?;

        if !ok {
            return Err(AppError::Unauthorized("Current password is incorrect.".into()));
        }

        let new_hash = self.password_hasher.hash(&new_password).await?;
        self.user_repo.update_password(user_id, &new_hash).await
    }

    // ── Forgot password ───────────────────────────────────────────────────────

    pub async fn forgot_password(&self, email: String) -> Result<(), AppError> {
        let email = email.trim().to_lowercase();

        let user = match self.user_repo.find_by_email(&email).await? {
            None => return Ok(()),
            Some(u) => u,
        };

        let token = generate_token();
        let expires_at = Utc::now() + Duration::hours(PASSWORD_RESET_EXPIRY_HOURS);

        self.user_repo
            .create_reset_token(NewResetToken {
                user_id: user.id,
                token: token.clone(),
                expires_at,
            })
            .await?;

        self.email.send_password_reset(&user.email, &token).await;

        Ok(())
    }

    // ── Reset password ────────────────────────────────────────────────────────

    pub async fn reset_password(
        &self,
        token: String,
        new_password: String,
    ) -> Result<(), AppError> {
        validate_password(&new_password)?;

        let reset_token = self
            .user_repo
            .find_valid_reset_token(&token)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(
                    "Password reset link is invalid or has expired.".into(),
                )
            })?;

        let new_hash = self.password_hasher.hash(&new_password).await?;
        self.user_repo
            .update_password(reset_token.user_id, &new_hash)
            .await?;
        self.user_repo.consume_reset_token(reset_token.id).await?;
        let _ = self.user_repo.reset_failed_login(reset_token.user_id).await;

        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn generate_token() -> String {
    let bytes: [u8; 32] = OsRng.gen();
    hex::encode(bytes)
}
