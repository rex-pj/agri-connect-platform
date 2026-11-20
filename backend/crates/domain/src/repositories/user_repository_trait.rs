use async_trait::async_trait;

use crate::{
    error::Error,
    models::{
        role::UserRole,
        user::{
            ExistingUser, LoginName, NewResetToken, NewUser, UserAdmin, UserAdminDetail,
            UserByEmail, UserCredentials, UserMe,
        },
    },
};

#[async_trait]
pub trait UserRepositoryTrait {
    /// The account a sign-in name names (its username or its email), active
    /// or not: the use case says why a deactivated one is refused.
    async fn find_credentials(&self, login: &LoginName) -> Result<Option<UserCredentials>, Error>;

    async fn find_credentials_by_id(&self, id: i32) -> Result<Option<UserCredentials>, Error>;

    async fn find_me(&self, user_id: i32) -> Result<Option<UserMe>, Error>;

    async fn count_by_role(&self, role: &str) -> Result<i64, Error>;

    async fn create(&self, user: NewUser) -> Result<i32, Error>;

    /// Creates `user` (an admin) only if no account with its role exists yet,
    /// atomically: of two concurrent calls, one creates and the other gets
    /// `None`.
    async fn create_first_admin(&self, user: NewUser) -> Result<Option<i32>, Error>;

    /// Self-registration: the provider account and the business it owns, both
    /// or neither. Returns the user id.
    async fn create_with_business(
        &self,
        user: NewUser,
        business_name: &str,
        business_phone: &str,
    ) -> Result<i32, Error>;

    async fn find_by_username(&self, username: &str) -> Result<Option<ExistingUser>, Error>;

    async fn find_by_email(&self, email: &str) -> Result<Option<UserByEmail>, Error>;

    async fn email_exists(&self, email: &str) -> Result<bool, Error>;

    async fn update_password(&self, user_id: i32, password_hash: &str) -> Result<(), Error>;

    /// Session tokens of `user_id` issued before now stop working.
    async fn end_sessions(&self, user_id: i32) -> Result<(), Error>;

    async fn create_reset_token(&self, token: NewResetToken) -> Result<(), Error>;

    /// The reset tokens of `user_id` that have not run out at `now`, used
    /// or not: how many links the account was sent lately.
    async fn count_live_reset_tokens(
        &self,
        user_id: i32,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, Error>;

    /// Whether a reset link with this hash would be taken now (unused, not
    /// run out): asked before the costly password hash, so a made-up link
    /// costs a lookup and nothing more.
    async fn reset_token_live(&self, token_hash: &str) -> Result<bool, Error>;

    /// Uses the reset link with this hash: marks it used, stores
    /// `password_hash` for its account and retires the account's other
    /// links, all or nothing; the account's id. None for a link that is
    /// unknown, spent or run out. Of two concurrent resets with the same
    /// link, exactly one gets it.
    async fn reset_password_with_token(
        &self,
        token_hash: &str,
        password_hash: &str,
    ) -> Result<Option<i32>, Error>;

    /// Deletes the emailed links (password resets, email confirmations) that
    /// ran out before `before`; how many. Nothing reads them again.
    async fn delete_expired_links(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, Error>;

    /// Stores a link that confirms the account's email address.
    async fn create_email_verification(&self, token: NewResetToken) -> Result<(), Error>;

    /// The confirmation links of `user_id` that have not run out at `now`,
    /// used or not.
    async fn count_live_email_verifications(
        &self,
        user_id: i32,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, Error>;

    /// Marks the unexpired, unused confirmation link with this hash as used
    /// and its account's address as verified, together; the account's id, or
    /// none for a link that is unknown, spent or run out.
    async fn claim_email_verification(&self, token_hash: &str) -> Result<Option<i32>, Error>;

    async fn count_all(&self) -> Result<i64, Error>;

    /// The admin list; `search` narrows it to accounts whose username, name
    /// or email contains it (accents and case ignored).
    async fn list_for_admin(
        &self,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<UserAdmin>, Error>;

    async fn count_for_admin(&self, search: Option<&str>) -> Result<i64, Error>;

    async fn find_by_id_for_admin(&self, id: i32) -> Result<Option<UserAdminDetail>, Error>;

    async fn update_role(&self, user_id: i32, role: &UserRole) -> Result<(), Error>;

    async fn toggle_active(&self, user_id: i32, is_active: bool) -> Result<(), Error>;

    /// Increment `failed_login_attempts`. If the new count reaches `max_attempts`,
    /// set `locked_until = NOW() + lockout_minutes`.
    async fn record_failed_login(
        &self,
        user_id: i32,
        max_attempts: i32,
        lockout_minutes: i64,
    ) -> Result<(), Error>;

    /// Reset `failed_login_attempts = 0` and clear `locked_until`.
    async fn reset_failed_login(&self, user_id: i32) -> Result<(), Error>;
}
