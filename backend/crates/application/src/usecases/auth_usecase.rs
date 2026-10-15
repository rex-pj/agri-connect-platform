use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use domain::{
    constants::{ROLE_ADMIN, ROLE_PROVIDER},
    models::{
        role::UserRole,
        session::NewRefreshToken,
        user::{NewResetToken, NewUser},
    },
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        permission_repository_trait::PermissionRepositoryTrait,
        refresh_token_repository_trait::RefreshTokenRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};
use rand::{rngs::OsRng, Rng};
use uuid::Uuid;

use crate::{
    authz::Actor,
    constants::{
        LOGIN_FAIL_MAX_ATTEMPTS, LOGIN_LOCK_MINUTES, PASSWORD_RESET_EXPIRY_HOURS,
        REFRESH_REUSE_GRACE_SECS,
    },
    outputs::{LoginResponse, MeResponse},
    ports::{EmailService, PasswordHasher, TokenService},
    validation::{validate_password, validate_phone, validate_username},
};

#[derive(Clone)]
pub struct AuthUseCase {
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
    refresh_repo: Arc<dyn RefreshTokenRepositoryTrait + Send + Sync>,
    email: Arc<dyn EmailService>,
    password_hasher: Arc<dyn PasswordHasher>,
    token_svc: Arc<dyn TokenService>,
    refresh_days: i64,
}

/// Everything `AuthUseCase` depends on, named so wiring cannot swap two of
/// compatible types by position.
pub struct AuthDeps {
    pub user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    pub business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    pub permission_repo: Arc<dyn PermissionRepositoryTrait + Send + Sync>,
    pub refresh_repo: Arc<dyn RefreshTokenRepositoryTrait + Send + Sync>,
    pub email: Arc<dyn EmailService>,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub token_svc: Arc<dyn TokenService>,
    /// REFRESH_TOKEN_DAYS: how long a browser session survives without use.
    pub refresh_days: i64,
}

/// A browser session: a short access token and the refresh token that
/// renews it. `refresh` is the raw value for the cookie; only its hash is
/// stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTokens {
    pub access: String,
    pub refresh: String,
    pub refresh_expires_at: DateTime<Utc>,
}

impl AuthUseCase {
    pub fn new(d: AuthDeps) -> Self {
        Self {
            user_repo: d.user_repo,
            business_repo: d.business_repo,
            permission_repo: d.permission_repo,
            refresh_repo: d.refresh_repo,
            email: d.email,
            password_hasher: d.password_hasher,
            token_svc: d.token_svc,
            refresh_days: d.refresh_days,
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
            user_id: credentials.id,
            token,
            username,
            role: credentials.role,
        })
    }

    // ── Browser sessions (refresh tokens) ─────────────────────────────────────

    /// A new session (a new token family) for a user who just proved who they
    /// are: login, registration, first-admin setup.
    pub async fn start_session(&self, user_id: i32) -> Result<SessionTokens> {
        self.issue(user_id, Uuid::new_v4()).await?.ok_or_else(|| {
            Error::internal(format!("user {user_id} vanished while starting a session"))
        })
    }

    /// Exchanges a refresh token for a fresh pair. `None` means "no session":
    /// unknown, expired, revoked, or already spent. A spent token shown again
    /// after the grace period means a copy exists elsewhere, so the whole
    /// family is revoked and both holders must log in again.
    pub async fn refresh_session(&self, raw: &str) -> Result<Option<SessionTokens>> {
        let Some(token) = self.refresh_repo.find_by_hash(&hash_token(raw)).await? else {
            return Ok(None);
        };
        let now = Utc::now();
        if token.revoked_at.is_some() || token.expires_at <= now {
            return Ok(None);
        }
        if let Some(used_at) = token.used_at {
            if now - used_at > Duration::seconds(REFRESH_REUSE_GRACE_SECS) {
                tracing::warn!(user_id = token.user_id, family = %token.family_id, "refresh token reused; revoking its family");
                self.refresh_repo.revoke_family(token.family_id).await?;
            }
            return Ok(None);
        }
        if !self.refresh_repo.claim(token.id).await? {
            // A concurrent request rotated it a moment ago.
            return Ok(None);
        }
        let issued = self.issue(token.user_id, token.family_id).await?;
        if issued.is_none() {
            self.refresh_repo.revoke_family(token.family_id).await?;
        }
        Ok(issued)
    }

    /// Logout: the session's whole family stops working at once.
    pub async fn end_session(&self, raw: &str) -> Result<()> {
        if let Some(token) = self.refresh_repo.find_by_hash(&hash_token(raw)).await? {
            self.refresh_repo.revoke_family(token.family_id).await?;
        }
        Ok(())
    }

    /// Deletes refresh tokens that expired; run periodically.
    pub async fn purge_expired_sessions(&self) -> Result<u64> {
        self.refresh_repo.delete_expired(Utc::now()).await
    }

    /// Both tokens for `user_id` in `family_id`. `None` when the account is
    /// gone, deactivated or has no role: no session for it.
    async fn issue(&self, user_id: i32, family_id: Uuid) -> Result<Option<SessionTokens>> {
        let Some(actor) = self.actor(user_id).await? else {
            return Ok(None);
        };
        let Some(role) = UserRole::from_code(&actor.role_code) else {
            return Ok(None);
        };
        let access = self.token_svc.mint(user_id, &actor.username, &role)?;
        let refresh = generate_token();
        let refresh_expires_at = Utc::now() + Duration::days(self.refresh_days);
        self.refresh_repo
            .insert(&NewRefreshToken {
                id: Uuid::new_v4(),
                family_id,
                user_id,
                token_hash: hash_token(&refresh),
                expires_at: refresh_expires_at,
            })
            .await?;
        Ok(Some(SessionTokens {
            access,
            refresh,
            refresh_expires_at,
        }))
    }

    // ── First-admin bootstrap ─────────────────────────────────────────────────

    /// True until the first admin exists: /login sends visitors to /setup and
    /// /setup refuses once it is false. Public on purpose: the setup page
    /// itself reveals the same fact.
    pub async fn needs_first_admin(&self) -> Result<bool> {
        Ok(self.user_repo.count_by_role(ROLE_ADMIN).await? == 0)
    }

    pub async fn create_first_admin(
        &self,
        username: String,
        email: String,
        password: String,
        full_name: Option<String>,
    ) -> Result<LoginResponse> {
        if !self.needs_first_admin().await? {
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
            user_id,
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
            user_id,
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
        // Whoever asked for the reset may be locking someone else out: every
        // existing session of the account ends here.
        self.refresh_repo.revoke_user(reset_token.user_id).await?;
        if let Err(e) = self.user_repo.reset_failed_login(reset_token.user_id).await {
            tracing::warn!(user_id = reset_token.user_id, error = %e, "could not reset failed-login counter");
        }
        Ok(())
    }
}

/// Refresh tokens are stored as this hash only; the raw value lives in the
/// browser's cookie.
fn hash_token(raw: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(raw.as_bytes()))
}

fn generate_token() -> String {
    let bytes: [u8; 32] = OsRng.gen();
    hex::encode(bytes)
}
