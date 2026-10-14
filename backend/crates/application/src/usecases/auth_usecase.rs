use std::sync::Arc;

use chrono::{Duration, Utc};
use domain::{
    constants::{ROLE_ADMIN, ROLE_PROVIDER},
    models::{
        role::UserRole,
        user::{NewResetToken, NewUser},
    },
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        permission_repository_trait::PermissionRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};
use rand::{rngs::OsRng, Rng};

use crate::{
    authz::Actor,
    constants::{LOGIN_FAIL_MAX_ATTEMPTS, LOGIN_LOCK_MINUTES, PASSWORD_RESET_EXPIRY_HOURS},
    outputs::{LoginResponse, MeResponse},
    ports::{EmailService, PasswordHasher, TokenService},
    validation::{validate_password, validate_phone, validate_username},
};

#[derive(Clone)]
pub struct AuthUseCase {
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
    email: Arc<dyn EmailService>,
    password_hasher: Arc<dyn PasswordHasher>,
    token_svc: Arc<dyn TokenService>,
}

impl AuthUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
        business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
        permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
        email: Arc<dyn EmailService>,
        password_hasher: Arc<dyn PasswordHasher>,
        token_svc: Arc<dyn TokenService>,
    ) -> Self {
        Self {
            user_repo,
            business_repo,
            permission_repo,
            email,
            password_hasher,
            token_svc,
        }
    }

    // ── Login ─────────────────────────────────────────────────────────────────

    /// The `Actor` behind a verified session token: `None` when the account no
    /// longer exists or was deactivated after the token was issued.
    pub async fn actor(&self, user_id: i32) -> Result<Option<Actor>> {
        Ok(self
            .permission_repo
            .find_actor(user_id)
            .await?
            .and_then(Actor::from_record))
    }

    pub async fn login(&self, username: String, password: String) -> Result<LoginResponse> {
        let username = username.trim().to_lowercase();

        let credentials = self
            .user_repo
            .find_credentials_by_username(&username)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::InvalidCredentials))?;

        if !credentials.is_active {
            return Err(Error::new(ErrorCode::AccountDeactivated));
        }

        if let Some(locked_until) = credentials.locked_until {
            if locked_until > Utc::now() {
                return Err(Error::new(ErrorCode::AccountLocked));
            }
        }

        if !self
            .password_hasher
            .verify(&password, &credentials.password_hash)
            .await?
        {
            // A failure to record the attempt must not turn a wrong password
            // into a 500, nor reveal anything; the lockout simply lags one try.
            if let Err(e) = self
                .user_repo
                .record_failed_login(credentials.id, LOGIN_FAIL_MAX_ATTEMPTS, LOGIN_LOCK_MINUTES)
                .await
            {
                tracing::warn!(user_id = credentials.id, error = %e, "could not record failed login");
            }
            return Err(Error::new(ErrorCode::InvalidCredentials));
        }

        if let Err(e) = self.user_repo.reset_failed_login(credentials.id).await {
            tracing::warn!(user_id = credentials.id, error = %e, "could not reset failed-login counter");
        }

        let role = UserRole::from_code(&credentials.role)
            .ok_or_else(|| Error::new(ErrorCode::AccountHasNoRole))?;

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
    ) -> Result<LoginResponse> {
        if self.user_repo.count_by_role(ROLE_ADMIN).await? > 0 {
            return Err(Error::new(ErrorCode::AdminAlreadyExists));
        }

        let username = username.trim().to_lowercase();
        Validation::new()
            .rule("username", validate_username(&username))
            .rule("password", validate_password(&password))
            .finish()?;

        let password_hash = self.password_hasher.hash(&password).await?;
        let user_id = self
            .user_repo
            .create(NewUser {
                username: username.clone(),
                email: email.trim().to_lowercase(),
                password_hash,
                role: ROLE_ADMIN.into(),
                full_name: Some(full_name.unwrap_or_else(|| "Administrator".to_owned())),
            })
            .await?;

        let token = self.token_svc.mint(user_id, &username, &UserRole::Admin)?;
        Ok(LoginResponse {
            token,
            username,
            role: ROLE_ADMIN.into(),
        })
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
    ) -> Result<LoginResponse> {
        let username = username.trim().to_lowercase();
        let email = email.trim().to_lowercase();
        let business_name = business_name.trim().to_owned();
        let business_phone = business_phone.trim().to_owned();

        Validation::new()
            .rule("username", validate_username(&username))
            .rule("password", validate_password(&password))
            .check(
                business_name.is_empty(),
                "business_name",
                ErrorCode::BusinessNameRequired,
            )
            .rule("business_phone", validate_phone(&business_phone))
            .finish()?;

        if self.user_repo.find_by_username(&username).await?.is_some() {
            return Err(Error::new(ErrorCode::UsernameTaken));
        }
        if self.user_repo.email_exists(&email).await? {
            return Err(Error::new(ErrorCode::EmailTaken));
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
            .await?;

        self.business_repo
            .create_for_registration(&business_name, &business_phone, user_id)
            .await?;

        let token = self
            .token_svc
            .mint(user_id, &username, &UserRole::Provider)?;
        Ok(LoginResponse {
            token,
            username,
            role: ROLE_PROVIDER.into(),
        })
    }

    // ── Me ────────────────────────────────────────────────────────────────────

    pub async fn me(&self, user_id: i32) -> Result<MeResponse> {
        let user = self
            .user_repo
            .find_me(user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::AccountNotFound))?;

        Ok(MeResponse {
            username: user.username,
            email: user.email,
            full_name: user.full_name,
            role: user.role,
            created_on: user.created_on.to_rfc3339(),
        })
    }

    // ── Change password ───────────────────────────────────────────────────────

    pub async fn change_password(
        &self,
        user_id: i32,
        current_password: String,
        new_password: String,
    ) -> Result<()> {
        Validation::new()
            .rule("new_password", validate_password(&new_password))
            .finish()?;

        let credentials = self
            .user_repo
            .find_credentials_by_id(user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::AccountNotFound))?;

        if !credentials.is_active {
            return Err(Error::new(ErrorCode::AccountDeactivated));
        }

        if !self
            .password_hasher
            .verify(&current_password, &credentials.password_hash)
            .await?
        {
            return Err(Error::new(ErrorCode::CurrentPasswordIncorrect));
        }

        let new_hash = self.password_hasher.hash(&new_password).await?;
        self.user_repo.update_password(user_id, &new_hash).await
    }

    // ── Forgot password ───────────────────────────────────────────────────────

    /// Always succeeds for a well-formed request: answering differently for
    /// unknown addresses would let anyone enumerate registered emails.
    pub async fn forgot_password(&self, email: String) -> Result<()> {
        let email = email.trim().to_lowercase();

        let Some(user) = self.user_repo.find_by_email(&email).await? else {
            return Ok(());
        };

        let token = generate_token();
        self.user_repo
            .create_reset_token(NewResetToken {
                user_id: user.id,
                token: token.clone(),
                expires_at: Utc::now() + Duration::hours(PASSWORD_RESET_EXPIRY_HOURS),
            })
            .await?;

        self.email.send_password_reset(&user.email, &token).await;
        Ok(())
    }

    // ── Reset password ────────────────────────────────────────────────────────

    pub async fn reset_password(&self, token: String, new_password: String) -> Result<()> {
        Validation::new()
            .rule("new_password", validate_password(&new_password))
            .finish()?;

        let reset_token = self
            .user_repo
            .find_valid_reset_token(&token)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ResetTokenInvalid))?;

        let new_hash = self.password_hasher.hash(&new_password).await?;
        self.user_repo
            .update_password(reset_token.user_id, &new_hash)
            .await?;
        self.user_repo.consume_reset_token(reset_token.id).await?;
        if let Err(e) = self.user_repo.reset_failed_login(reset_token.user_id).await {
            tracing::warn!(user_id = reset_token.user_id, error = %e, "could not reset failed-login counter");
        }
        Ok(())
    }
}

fn generate_token() -> String {
    let bytes: [u8; 32] = OsRng.gen();
    hex::encode(bytes)
}
