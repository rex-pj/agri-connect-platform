use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use domain::{
    constants::{ROLE_ADMIN, ROLE_PROVIDER},
    events::{AuthEvent, Change, DomainEvent, Entity},
    models::{
        role::UserRole,
        session::NewRefreshToken,
        user::{LoginName, NewResetToken, NewUser},
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
        BUSINESS_NAME_MAX, EMAIL_VERIFICATION_EXPIRY_HOURS, EMAIL_VERIFICATION_LINKS_MAX,
        FULL_NAME_MAX, LOGIN_FAIL_ACCOUNT_MAX_ATTEMPTS, LOGIN_FAIL_MAX_ATTEMPTS,
        LOGIN_LOCK_MINUTES, PASSWORD_RESET_EXPIRY_HOURS, PASSWORD_RESET_LINKS_MAX,
        REFRESH_REUSE_GRACE_SECS, SIGNUP_ATTEMPTS_PER_CLIENT, SIGNUP_CLIENT_WINDOW_SECS,
    },
    hashing::sha256_hex,
    inputs::RegisterInput,
    outputs::{LoginResponse, MeResponse},
    ports::{EmailService, EventBus, PasswordHasher, SessionClaims, TokenService},
    repeats::WindowCounter,
    settings::SettingsReader,
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
    /// The audit log hears who signed in, who was kept out and when a
    /// password changed.
    events: Arc<dyn EventBus>,
    refresh_days: i64,
    setup_token: Option<String>,
    /// Whether providers may sign themselves up (`access.registration_open`).
    settings: SettingsReader,
    /// A hash no password matches, made once with the real hasher: a login
    /// for an unknown username verifies against it, so it takes as long as
    /// one for a known username.
    decoy_hash: Arc<tokio::sync::OnceCell<String>>,
    /// Wrong passwords per account and client, in memory only.
    failed_logins: WindowCounter,
    /// Sign-up tries per client (hashed address), in memory only.
    signups_by_client: WindowCounter,
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
    pub events: Arc<dyn EventBus>,
    /// REFRESH_TOKEN_DAYS: how long a browser session survives without use.
    pub refresh_days: i64,
    /// SETUP_TOKEN: when set, creating the first admin needs it.
    pub setup_token: Option<String>,
    pub settings: SettingsReader,
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
            events: d.events,
            refresh_days: d.refresh_days,
            setup_token: d.setup_token,
            settings: d.settings,
            decoy_hash: Arc::default(),
            failed_logins: WindowCounter::default(),
            signups_by_client: WindowCounter::default(),
        }
    }

    /// Writes `what` about the account `user_id` to the audit log: the
    /// account is both who and what the entry is about.
    async fn audit(&self, user_id: i32, what: AuthEvent) {
        self.events
            .publish(DomainEvent {
                actor_id: Some(user_id),
                entity: Entity::User,
                entity_id: user_id,
                change: Change::Auth(what),
                reason: None,
            })
            .await;
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
        // Logged out (or the account was deactivated, or its refresh token
        // was reused): the access token goes with its session, not an hour
        // later.
        if !self.refresh_repo.family_live(claims.session).await? {
            return Err(Error::new(ErrorCode::InvalidToken).with_detail("session ended"));
        }
        Ok(Actor::from_record(record))
    }

    /// `login` is the account's username or its email (`LoginName`: with an
    /// `@` an email); either names the same account, so the failure counts
    /// and locks below are shared.
    ///
    /// Whoever does not know the password learns nothing about the account:
    /// an unknown name, a deactivated account and a locked one all answer
    /// `invalid_credentials`, after the same password check. Only the right
    /// password is told that the account is deactivated or locked.
    ///
    /// `client` tells callers apart (the address, an IPv6 /64; never
    /// stored): `LOGIN_FAIL_MAX_ATTEMPTS` wrong passwords keep that client
    /// out of that account for `LOGIN_LOCK_MINUTES`, while the owner on
    /// another address still gets in. The account itself is locked only
    /// after `LOGIN_FAIL_ACCOUNT_MAX_ATTEMPTS` from every client together.
    pub async fn login(
        &self,
        login: String,
        password: String,
        client: &str,
    ) -> Result<LoginResponse> {
        let Some(credentials) = self
            .user_repo
            .find_credentials(&LoginName::parse(&login))
            .await?
        else {
            let decoy = self
                .decoy_hash
                .get_or_try_init(|| async { self.password_hasher.hash(&generate_token()).await })
                .await?;
            self.password_hasher.verify(&password, decoy).await?;
            return Err(Error::new(ErrorCode::InvalidCredentials));
        };
        let client_key = sha256_hex(format!("{}|{client}", credentials.id));
        let lock_window = std::time::Duration::from_secs(LOGIN_LOCK_MINUTES as u64 * 60);
        let locked = credentials
            .locked_until
            .is_some_and(|until| until > Utc::now())
            || self.failed_logins.count(&client_key, lock_window) >= LOGIN_FAIL_MAX_ATTEMPTS;

        if !self
            .password_hasher
            .verify(&password, &credentials.password_hash)
            .await?
        {
            // Not counted while locked (each guess would push the lock
            // further) nor for a deactivated account. A failure to record the
            // attempt must not turn a wrong password into a 500, nor reveal
            // anything; the account-wide lock simply lags one try.
            if credentials.is_active && !locked {
                self.failed_logins.hit(client_key, lock_window);
                if let Err(e) = self
                    .user_repo
                    .record_failed_login(
                        credentials.id,
                        LOGIN_FAIL_ACCOUNT_MAX_ATTEMPTS,
                        LOGIN_LOCK_MINUTES,
                    )
                    .await
                {
                    tracing::warn!(user_id = credentials.id, error = %e, "could not record failed login");
                }
                self.audit(credentials.id, AuthEvent::LoginFailed).await;
            }
            return Err(Error::new(ErrorCode::InvalidCredentials));
        }

        if !credentials.is_active {
            return Err(Error::new(ErrorCode::AccountDeactivated));
        }
        if locked {
            return Err(Error::new(ErrorCode::AccountLocked));
        }

        self.failed_logins.clear(&client_key);
        if let Err(e) = self.user_repo.reset_failed_login(credentials.id).await {
            tracing::warn!(user_id = credentials.id, error = %e, "could not reset failed-login counter");
        }

        // An account whose role is unknown has no permissions to act with.
        if UserRole::from_code(&credentials.role).is_none() {
            return Err(Error::new(ErrorCode::AccountHasNoRole));
        }

        let response = self
            .signed_in(credentials.id, credentials.username, credentials.role)
            .await?;
        self.audit(credentials.id, AuthEvent::LoggedIn).await;
        Ok(response)
    }

    /// The answer to a login, sign-up or first-admin setup: a new session,
    /// whose access token is the API's `token`. No token is ever minted
    /// outside a session, so every one can be ended before it expires.
    async fn signed_in(
        &self,
        user_id: i32,
        username: String,
        role: String,
    ) -> Result<LoginResponse> {
        let session = self.start_session(user_id).await?;
        Ok(LoginResponse {
            user_id,
            token: session.access.clone(),
            session,
            username,
            role,
        })
    }

    /// Ends the session a token belongs to (`DELETE /api/auth/tokens`, a
    /// script logging out): the token and every one renewed from it stop
    /// working on the next request. A session already ended is no error.
    pub async fn end_token_session(&self, claims: &SessionClaims) -> Result<()> {
        self.refresh_repo.revoke_family(claims.session).await
    }

    // ── Browser sessions (refresh tokens) ─────────────────────────────────────

    /// A new session (a new token family) for a user who just proved who they
    /// are: login, registration, first-admin setup.
    pub async fn start_session(&self, user_id: i32) -> Result<SessionTokens> {
        self.issue(user_id, Uuid::new_v4()).await?.ok_or_else(|| {
            Error::internal(format!("user {user_id} vanished while starting a session"))
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
                self.audit(token.user_id, AuthEvent::SessionReused).await;
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

    /// Deletes refresh tokens and emailed links (password resets, email
    /// confirmations) that expired; run periodically.
    pub async fn purge_expired_sessions(&self) -> Result<u64> {
        let now = Utc::now();
        Ok(self.refresh_repo.delete_expired(now).await?
            + self.user_repo.delete_expired_links(now).await?)
    }

    /// Forgets the wrong passwords and sign-up tries whose window has passed
    /// (bounded memory); called with the other in-memory filters.
    pub fn forget_old_attempts(&self) {
        self.failed_logins
            .forget_older_than(std::time::Duration::from_secs(
                LOGIN_LOCK_MINUTES as u64 * 60,
            ));
        self.signups_by_client
            .forget_older_than(std::time::Duration::from_secs(SIGNUP_CLIENT_WINDOW_SECS));
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
        let access = self.token_svc.mint(user_id, &actor.username, family_id)?;
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

    /// Whether the setup form must ask for the setup token (SETUP_TOKEN).
    pub fn setup_token_required(&self) -> bool {
        self.setup_token.is_some()
    }

    /// `setup_token`: what the caller typed; checked only when SETUP_TOKEN is
    /// configured, so a fresh installation belongs to whoever deployed it and
    /// not to the first visitor.
    pub async fn create_first_admin(
        &self,
        username: String,
        email: String,
        password: String,
        full_name: Option<String>,
        setup_token: Option<String>,
    ) -> Result<LoginResponse> {
        if !self.needs_first_admin().await? {
            return Err(Error::new(ErrorCode::AdminAlreadyExists));
        }
        if let Some(expected) = &self.setup_token {
            let given = setup_token.as_deref().map(str::trim).unwrap_or_default();
            // Hashes, so the comparison's time says nothing about the token.
            if sha256_hex(given) != sha256_hex(expected) {
                return Err(Error::field("setup_token", ErrorCode::SetupTokenInvalid));
            }
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
                email_verified: true,
            })
            .await?
            .ok_or_else(|| Error::new(ErrorCode::AdminAlreadyExists))?;

        self.signed_in(user_id, username, ROLE_ADMIN.into()).await
    }

    // ── Provider self-registration ────────────────────────────────────────────

    /// A provider's own account and business. Whether the username or the
    /// email is taken is told (the form must say which box to change), so
    /// one client may try at most `SIGNUP_ATTEMPTS_PER_CLIENT` valid
    /// sign-ups a day (`signup_daily_limit`): nobody can sweep a list of
    /// addresses through it.
    pub async fn register(&self, input: RegisterInput) -> Result<LoginResponse> {
        if !self.settings.load().await?.registration_open() {
            return Err(Error::new(ErrorCode::RegistrationClosed));
        }
        let window = std::time::Duration::from_secs(SIGNUP_CLIENT_WINDOW_SECS);
        // Hashed: the counter never holds an address.
        let client_key = (!input.client.is_empty()).then(|| sha256_hex(&input.client));
        if client_key
            .as_deref()
            .is_some_and(|k| self.signups_by_client.count(k, window) >= SIGNUP_ATTEMPTS_PER_CLIENT)
        {
            return Err(Error::new(ErrorCode::SignupDailyLimit));
        }
        let username = input.username.trim().to_lowercase();
        let email = input.email.trim().to_lowercase();
        let business_name = input.business_name.trim().to_owned();
        let business_phone = input.business_phone.trim().to_owned();
        let full_name = non_blank(input.full_name);
        let password = input.password;

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

        // From here on a try learns whether the username or email is taken.
        if let Some(key) = client_key {
            self.signups_by_client.hit(key, window);
        }
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
                    email: email.clone(),
                    password_hash,
                    role: ROLE_PROVIDER.into(),
                    full_name,
                    email_verified: false,
                },
                &business_name,
                &business_phone,
            )
            .await?;
        // The account exists either way; a link that could not be made is
        // asked for again from the account page.
        if let Err(e) = self.send_email_verification(user_id, email).await {
            tracing::warn!(user_id, error = %e, "could not send the email confirmation link");
        }

        self.signed_in(user_id, username, ROLE_PROVIDER.into())
            .await
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
            email_verified: user.email_verified,
        })
    }

    // ── Email confirmation ────────────────────────────────────────────────────

    /// Emails `to` a link that confirms it is the account's address, unless
    /// `EMAIL_VERIFICATION_LINKS_MAX` links are still valid: the form must
    /// not fill a stranger's inbox. Sent in the background.
    async fn send_email_verification(&self, user_id: i32, to: String) -> Result<()> {
        let now = Utc::now();
        if self
            .user_repo
            .count_live_email_verifications(user_id, now)
            .await?
            >= EMAIL_VERIFICATION_LINKS_MAX
        {
            tracing::info!(
                user_id,
                "email confirmation not sent: enough links are still valid"
            );
            return Ok(());
        }
        let token = generate_token();
        self.user_repo
            .create_email_verification(NewResetToken {
                user_id,
                token_hash: hash_token(&token),
                expires_at: now + Duration::hours(EMAIL_VERIFICATION_EXPIRY_HOURS),
            })
            .await?;
        let email = Arc::clone(&self.email);
        tokio::spawn(async move { email.send_email_verification(&to, &token).await });
        Ok(())
    }

    /// The signed-in account asks for its confirmation link again. Nothing
    /// to do for an address already confirmed.
    pub async fn request_email_verification(&self, user_id: i32) -> Result<()> {
        let user = self
            .user_repo
            .find_me(user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::AccountNotFound))?;
        if user.email_verified {
            return Ok(());
        }
        self.send_email_verification(user_id, user.email).await
    }

    /// Opens a confirmation link: its account's address is verified from now
    /// on. A link that is unknown, already used or run out is
    /// `email_verification_invalid`.
    pub async fn verify_email(&self, token: &str) -> Result<()> {
        let user_id = self
            .user_repo
            .claim_email_verification(&hash_token(token.trim()))
            .await?
            .ok_or_else(|| Error::new(ErrorCode::EmailVerificationInvalid))?;
        self.audit(user_id, AuthEvent::EmailVerified).await;
        Ok(())
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

        // A session in other hands must not guess the password it does not
        // know: `LOGIN_FAIL_MAX_ATTEMPTS` wrong ones stop the checks for
        // `LOGIN_LOCK_MINUTES` (before bcrypt, so guesses cost us nothing).
        let attempts_key = sha256_hex(format!("{user_id}|change_password"));
        let lock_window = std::time::Duration::from_secs(LOGIN_LOCK_MINUTES as u64 * 60);
        if self.failed_logins.count(&attempts_key, lock_window) >= LOGIN_FAIL_MAX_ATTEMPTS {
            return Err(Error::new(ErrorCode::TooManyRequests));
        }
        if !self
            .password_hasher
            .verify(&current_password, &credentials.password_hash)
            .await?
        {
            self.failed_logins.hit(attempts_key, lock_window);
            return Err(Error::new(ErrorCode::CurrentPasswordIncorrect));
        }
        self.failed_logins.clear(&attempts_key);

        let new_hash = self.password_hasher.hash(&new_password).await?;
        self.user_repo.update_password(user_id, &new_hash).await?;
        // A changed password is often a suspicion: every signed-in device is
        // signed out, now (access tokens) and for good (refresh tokens). The
        // browser that made the change gets a new session from its caller
        // (web::pages::account).
        self.user_repo.end_sessions(user_id).await?;
        self.refresh_repo.revoke_user(user_id).await?;
        self.audit(user_id, AuthEvent::PasswordChanged).await;
        Ok(())
    }

    // ── Forgot password ───────────────────────────────────────────────────────

    /// Always succeeds for a well-formed request: answering differently for
    /// unknown addresses would let anyone enumerate registered emails.
    pub async fn forgot_password(&self, email: String) -> Result<()> {
        let email = email.trim().to_lowercase();

        let Some(user) = self.user_repo.find_by_email(&email).await? else {
            return Ok(());
        };

        // Nobody needs more links than this at once, and without a cap the
        // form would fill a stranger's inbox.
        let now = Utc::now();
        if self.user_repo.count_live_reset_tokens(user.id, now).await? >= PASSWORD_RESET_LINKS_MAX {
            tracing::info!(
                user_id = user.id,
                "password-reset email not sent: enough links are still valid"
            );
            return Ok(());
        }

        let token = generate_token();
        self.user_repo
            .create_reset_token(NewResetToken {
                user_id: user.id,
                token_hash: hash_token(&token),
                expires_at: now + Duration::hours(PASSWORD_RESET_EXPIRY_HOURS),
            })
            .await?;

        // In the background: waiting for the mail server here would make a
        // known address answer slower than an unknown one.
        let email = Arc::clone(&self.email);
        tokio::spawn(async move { email.send_password_reset(&user.email, &token).await });
        Ok(())
    }

    // ── Reset password ────────────────────────────────────────────────────────

    pub async fn reset_password(&self, token: String, new_password: String) -> Result<()> {
        Validation::new()
            .rule("new_password", validate_password(&new_password))
            .finish()?;

        // A made-up link is refused for the price of a lookup, before the
        // costly hash; the claim below is still what decides.
        let token_hash = hash_token(token.trim());
        if !self.user_repo.reset_token_live(&token_hash).await? {
            return Err(Error::new(ErrorCode::ResetTokenInvalid));
        }
        // Hashed before the link is claimed: the slow step must not sit
        // between claiming it and storing the password.
        let new_hash = self.password_hasher.hash(&new_password).await?;
        // The link is spent, the password stored and the account's other
        // links retired together; a link sent twice at once resets once.
        let user_id = self
            .user_repo
            .reset_password_with_token(&token_hash, &new_hash)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ResetTokenInvalid))?;

        // Whoever asked for the reset may be locking someone else out: every
        // existing session of the account ends here.
        self.user_repo.end_sessions(user_id).await?;
        self.refresh_repo.revoke_user(user_id).await?;
        if let Err(e) = self.user_repo.reset_failed_login(user_id).await {
            tracing::warn!(user_id, error = %e, "could not reset failed-login counter");
        }
        self.audit(user_id, AuthEvent::PasswordReset).await;
        Ok(())
    }
}

/// Refresh tokens are stored as this hash only; the raw value lives in the
/// browser's cookie.
fn hash_token(raw: &str) -> String {
    sha256_hex(raw)
}

fn generate_token() -> String {
    let bytes: [u8; 32] = OsRng.gen();
    hex::encode(bytes)
}
