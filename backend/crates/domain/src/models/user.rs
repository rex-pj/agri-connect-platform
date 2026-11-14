use chrono::{DateTime, Utc};

pub struct UserCredentials {
    pub id: i32,
    pub password_hash: String,
    pub role: String,
    /// Whether the account is active. Use cases must reject login for
    /// inactive accounts — this field makes that invariant explicit at the
    /// domain model level rather than relying on a silent SQL filter.
    pub is_active: bool,
    pub locked_until: Option<DateTime<Utc>>,
}

pub struct NewUser {
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub full_name: Option<String>,
    /// The address is taken as the account's own from the start: an admin
    /// typed it for an account they create. A sign-up's is not, until the
    /// link sent to it is opened.
    pub email_verified: bool,
}

pub struct ExistingUser {
    pub id: i32,
    pub role: String,
}

pub struct UserAdmin {
    pub id: i32,
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub role_code: String,
    pub role_name: String,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
    pub created_by_name: Option<String>,
    pub updated_by_name: Option<String>,
}

pub struct UserAdminDetail {
    pub id: i32,
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub role_code: String,
    pub role_name: String,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
}

/// The authenticated user's own profile — returned by GET /api/auth/me.
pub struct UserMe {
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub role: String,
    pub created_on: chrono::DateTime<chrono::Utc>,
    /// The link sent to `email` was opened.
    pub email_verified: bool,
}

/// Minimal user info looked up by email for password-reset flows.
pub struct UserByEmail {
    pub id: i32,
    pub email: String,
}

/// A link emailed to an account: a password reset, or the confirmation of
/// its address (each kind in its own table).
pub struct NewResetToken {
    pub user_id: i32,
    /// SHA-256 of the emailed token; the raw value is never stored.
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}

/// Who a session belongs to and what they may do, loaded fresh on every
/// request so a revoked permission or a deactivated account takes effect at
/// once instead of when the session token expires.
pub struct ActorRecord {
    pub user_id: i32,
    pub username: String,
    pub role_code: String,
    pub is_active: bool,
    pub permissions: Vec<String>,
    /// Session tokens issued before this were ended (password changed).
    pub sessions_valid_after: Option<DateTime<Utc>>,
}
