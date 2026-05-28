use async_trait::async_trait;

use crate::{
    domain::models::{
        role::UserRole,
        user::{ExistingUser, NewResetToken, NewUser, ResetToken, UserAdmin, UserAdminDetail, UserByEmail, UserCredentials, UserMe},
    },
    view_models::AppError,
};

#[async_trait]
pub trait UserRepositoryTrait {
    async fn find_credentials_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserCredentials>, AppError>;

    async fn find_credentials_by_id(&self, id: i32) -> Result<Option<UserCredentials>, AppError>;

    async fn find_me(&self, user_id: i32) -> Result<Option<UserMe>, AppError>;

    async fn count_by_role(&self, role: &str) -> Result<i64, AppError>;

    async fn create(&self, user: NewUser) -> Result<i32, AppError>;

    async fn find_by_username(&self, username: &str) -> Result<Option<ExistingUser>, AppError>;

    async fn find_by_email(&self, email: &str) -> Result<Option<UserByEmail>, AppError>;

    async fn email_exists(&self, email: &str) -> Result<bool, AppError>;

    async fn update_password(&self, user_id: i32, password_hash: &str) -> Result<(), AppError>;

    async fn create_reset_token(&self, token: NewResetToken) -> Result<(), AppError>;

    async fn find_valid_reset_token(&self, token: &str) -> Result<Option<ResetToken>, AppError>;

    async fn consume_reset_token(&self, token_id: i32) -> Result<(), AppError>;

    async fn count_all(&self) -> Result<i64, AppError>;

    async fn list_for_admin(&self, limit: i64, offset: i64) -> Result<Vec<UserAdmin>, AppError>;

    async fn find_by_id_for_admin(&self, id: i32) -> Result<Option<UserAdminDetail>, AppError>;

    async fn update_role(&self, user_id: i32, role: &UserRole) -> Result<(), AppError>;

    async fn toggle_active(&self, user_id: i32, is_active: bool) -> Result<(), AppError>;

    /// Increment `failed_login_attempts`. If the new count reaches `max_attempts`,
    /// set `locked_until = NOW() + lockout_minutes`.
    async fn record_failed_login(
        &self,
        user_id:         i32,
        max_attempts:    i32,
        lockout_minutes: i64,
    ) -> Result<(), AppError>;

    /// Reset `failed_login_attempts = 0` and clear `locked_until`.
    async fn reset_failed_login(&self, user_id: i32) -> Result<(), AppError>;
}
