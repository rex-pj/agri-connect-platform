use async_trait::async_trait;

use crate::{
    error::Error,
    models::{
        role::UserRole,
        user::{
            ExistingUser, NewResetToken, NewUser, ResetToken, UserAdmin, UserAdminDetail,
            UserByEmail, UserCredentials, UserMe,
        },
    },
};

#[async_trait]
pub trait UserRepositoryTrait {
    async fn find_credentials_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserCredentials>, Error>;

    async fn find_credentials_by_id(&self, id: i32) -> Result<Option<UserCredentials>, Error>;

    async fn find_me(&self, user_id: i32) -> Result<Option<UserMe>, Error>;

    async fn count_by_role(&self, role: &str) -> Result<i64, Error>;

    async fn create(&self, user: NewUser) -> Result<i32, Error>;

    /// Creates `user` (an admin) only if no account with its role exists yet,
    /// atomically: of two concurrent calls, one creates and the other gets
    /// `None`.
    async fn create_first_admin(&self, user: NewUser) -> Result<Option<i32>, Error>;

    async fn find_by_username(&self, username: &str) -> Result<Option<ExistingUser>, Error>;

    async fn find_by_email(&self, email: &str) -> Result<Option<UserByEmail>, Error>;

    async fn email_exists(&self, email: &str) -> Result<bool, Error>;

    async fn update_password(&self, user_id: i32, password_hash: &str) -> Result<(), Error>;

    /// Session tokens of `user_id` issued before now stop working.
    async fn end_sessions(&self, user_id: i32) -> Result<(), Error>;

    async fn create_reset_token(&self, token: NewResetToken) -> Result<(), Error>;

    /// Marks the unexpired, unused token with this hash as used and returns
    /// it, in one statement: of two concurrent resets with the same link,
    /// exactly one gets it.
    async fn claim_reset_token(&self, token_hash: &str) -> Result<Option<ResetToken>, Error>;

    /// Marks every unused reset token of `user_id` as used, so older links
    /// stop working once the password was reset.
    async fn retire_reset_tokens(&self, user_id: i32) -> Result<(), Error>;

    async fn count_all(&self) -> Result<i64, Error>;

    async fn list_for_admin(&self, limit: i64, offset: i64) -> Result<Vec<UserAdmin>, Error>;

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
