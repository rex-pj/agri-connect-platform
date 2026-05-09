use chrono::{DateTime, Utc};

pub struct UserCredentials {
    pub id:            i32,
    pub password_hash: String,
    pub role:          String,
    pub locked_until:  Option<DateTime<Utc>>,
}

pub struct NewUser {
    pub username:      String,
    pub email:         String,
    pub password_hash: String,
    pub role:          String,
    pub full_name:     Option<String>,
}

pub struct ExistingUser {
    pub id:   i32,
    pub role: String,
}

pub struct UserAdmin {
    pub id:        i32,
    pub username:  String,
    pub email:     String,
    pub full_name: Option<String>,
    pub role_code: String,
    pub role_name: String,
    pub is_active: bool,
}

pub struct UserAdminDetail {
    pub id:         i32,
    pub username:   String,
    pub email:      String,
    pub full_name:  Option<String>,
    pub role_code:  String,
    pub role_name:  String,
    pub is_active:  bool,
    pub created_on: DateTime<Utc>,
}

/// The authenticated user's own profile — returned by GET /api/auth/me.
pub struct UserMe {
    pub username:   String,
    pub email:      String,
    pub full_name:  Option<String>,
    pub created_on: chrono::DateTime<chrono::Utc>,
}

/// Minimal user info looked up by email for password-reset flows.
pub struct UserByEmail {
    pub id:    i32,
    pub email: String,
}

/// A valid (unexpired, unused) reset token row.
pub struct ResetToken {
    pub id:      i32,
    pub user_id: i32,
}

pub struct NewResetToken {
    pub user_id:    i32,
    pub token:      String,
    pub expires_at: DateTime<Utc>,
}
