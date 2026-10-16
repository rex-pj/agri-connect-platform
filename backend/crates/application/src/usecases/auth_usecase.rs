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
        BUSINESS_NAME_MAX, FULL_NAME_MAX, LOGIN_FAIL_MAX_ATTEMPTS, LOGIN_LOCK_MINUTES,
        PASSWORD_RESET_EXPIRY_HOURS, REFRESH_REUSE_GRACE_SECS,
    },
    outputs::{LoginResponse, MeResponse},
    ports::{EmailService, PasswordHasher, SessionClaims, TokenService},
    validation::{
        max_chars, max_chars_opt, non_blank, validate_email, validate_password, validate_phone,
        validate_username,
    },
};

#[derive(Clone)]
pub struct AuthUseCase {
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
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

/// What became of a refresh token presented for renewal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Renewal {
    Renewed(SessionTokens),
    /// A concurrent request rotated it a moment ago: no new pair here, but
    /// the session lives on in the cookies that request set.
    Raced,
    /// Unknown, expired, revoked, reused or its account is gone.
    Ended,
}

impl AuthUseCase {
    pub fn new(d: AuthDeps) -> Self {
        Self {
            user_repo: d.user_repo,
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

    /// The `Actor` behind a verified session token, like `actor`, except that
    /// a token minted before the account's sessions were ended (its password
    /// changed or was reset) is `invalid_token`: the other devices must log in
    /// again now, not when their access token runs out.
    pub async fn session_actor(&self, claims: &SessionClaims) -> Result<Option<Actor>> {
        let Some(record) = self.permission_repo.find_actor(claims.user_id).await? else {
            return Ok(None);
        };
        // Whole seconds: `iat` has no fraction, and the token this browser
        // got with the change was minted within the same second.
        if record
            .sessions_valid_after
            .is_some_and(|cutoff| claims.issued_at < cutoff.timestamp())
        {
            return Err(Error::new(ErrorCode::InvalidToken).with_detail("session ended"));
        }
        Ok(Actor::from_record(record))
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

        // An account whose role is unknown has no permissions to act with.
        if UserRole::from_code(&credentials.role).is_none() {
            return Err(Error::new(ErrorCode::AccountHasNoRole));
        }

        let token = self.token_svc.mint(credentials.id, &username)?;
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

    /// Exchanges a refresh token for a fresh pair; `None` when there is no
    /// new pair for any reason (see `renew_session` for which).
    pub async fn refresh_session(&self, raw: &str) -> Result<Option<SessionTokens>> {
        Ok(match self.renew_session(raw).await? {
            Renewal::Renewed(tokens) => Some(tokens),
            Renewal::Raced | Renewal::Ended => None,
        })
    }

    /// Exchanges a refresh token for a fresh pair. A spent token shown again
    /// within the grace period is a concurrent request (two tabs) that lost
    /// the race: `Raced`, and the winner's cookies must be left alone. After
    /// the grace period it means a copy exists elsewhere, so the whole family
    /// is revoked and both holders must log in again.
    pub async fn renew_session(&self, raw: &str) -> Result<Renewal> {
        let Some(token) = self.refresh_repo.find_by_hash(&hash_token(raw)).await? else {
            return Ok(Renewal::Ended);
        };
        let now = Utc::now();
        if token.revoked_at.is_some() || token.expires_at <= now {
            return Ok(Renewal::Ended);
        }
        if let Some(used_at) = token.used_at {
            if now - used_at > Duration::seconds(REFRESH_REUSE_GRACE_SECS) {
                tracing::warn!(user_id = token.user_id, family = %token.family_id, "refresh token reused; revoking its family");
                self.refresh_repo.revoke_family(token.family_id).await?;
                return Ok(Renewal::Ended);
            }
            return Ok(Renewal::Raced);
        }
        if !self.refresh_repo.claim(token.id).await? {
            // A concurrent request rotated it a moment ago.
            return Ok(Renewal::Raced);
        }
        match self.issue(token.user_id, token.family_id).await? {
            Some(tokens) => Ok(Renewal::Renewed(tokens)),
            None => {
                self.refresh_repo.revoke_family(token.family_id).await?;
                Ok(Renewal::Ended)
            }
        }
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
        if UserRole::from_code(&actor.role_code).is_none() {
            return Ok(None);
        }
        let access = self.token_svc.mint(user_id, &actor.username)?;
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
        let email = email.trim().to_lowercase();
        let full_name = non_blank(full_name);
        Validation::new()
            .rule("username", validate_username(&username))
            .rule("email", validate_email(&email))
            .rule("password", validate_password(&password))
            .rule("full_name", max_chars_opt(&full_name, FULL_NAME_MAX))
            .finish()?;

        let password_hash = self.password_hasher.hash(&password).await?;
        // The check above answers early; this one is the guarantee, under a
        // lock, for two setup forms sent at the same moment.
        let user_id = self
            .user_repo
            .create_first_admin(NewUser {
                username: username.clone(),
                email,
                password_hash,
                role: ROLE_ADMIN.into(),
                full_name: Some(full_name.unwrap_or_else(|| "Administrator".to_owned())),
            })
            .await?
            .ok_or_else(|| Error::new(ErrorCode::AdminAlreadyExists))?;

        let token = self.token_svc.mint(user_id, &username)?;
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
        let full_name = non_blank(full_name);

        Validation::new()
            .rule("username", validate_username(&username))
            .rule("email", validate_email(&email))
            .rule("password", validate_password(&password))
            .rule("full_name", max_chars_opt(&full_name, FULL_NAME_MAX))
            .check(
                business_name.is_empty(),
                "business_name",
                ErrorCode::BusinessNameRequired,
            )
            .rule(
                "business_name",
                max_chars(&business_name, BUSINESS_NAME_MAX),
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
        // The account and its business in one transaction: never an account
        // that can log in but has nothing to manage (and keeps its username).
        let user_id = self
            .user_repo
            .create_with_business(
                NewUser {
                    username: username.clone(),
                    email,
                    password_hash,
                    role: ROLE_PROVIDER.into(),
                    full_name,
                },
                &business_name,
                &business_phone,
            )
            .await?;

        let token = self.token_svc.mint(user_id, &username)?;
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
        self.user_repo.update_password(user_id, &new_hash).await?;
        // A changed password is often a suspicion: every signed-in device is
        // signed out, now (access tokens) and for good (refresh tokens). The
        // browser that made the change gets a new session from its caller
        // (web::pages::account).
        self.user_repo.end_sessions(user_id).await?;
        self.refresh_repo.revoke_user(user_id).await
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
                token_hash: hash_token(&token),
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

        // Hash first: the slow step must not sit between claiming the token
        // and using it.
        let new_hash = self.password_hasher.hash(&new_password).await?;
        // Claimed before the password changes, so a link sent twice at once
        // resets the password once.
        let reset_token = self
            .user_repo
            .claim_reset_token(&hash_token(token.trim()))
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ResetTokenInvalid))?;

        self.user_repo
            .update_password(reset_token.user_id, &new_hash)
            .await?;
        // Any other link still in the inbox stops working too.
        self.user_repo
            .retire_reset_tokens(reset_token.user_id)
            .await?;
        // Whoever asked for the reset may be locking someone else out: every
        // existing session of the account ends here.
        self.user_repo.end_sessions(reset_token.user_id).await?;
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
