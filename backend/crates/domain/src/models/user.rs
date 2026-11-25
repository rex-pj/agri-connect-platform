use chrono::{DateTime, Utc};

/// An account is named by its email alone: the sign-in box, the "assign an
/// account" forms and every page that names an account take it (stored
/// trimmed and lowercased, UNIQUE). A page shows the full name when there is
/// one (`display_name`).
pub fn display_name(full_name: Option<&str>, email: &str) -> String {
    full_name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(email)
        .to_owned()
}

/// `tu.moi@gmail.com` as `t*****@gmail.com`: what the former address is
/// told the account moved to, enough for its owner to recognise and not
/// enough to write to, since the former address may be someone else's.
pub fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) => {
            let mut chars = local.chars();
            let first = chars.next().map(String::from).unwrap_or_default();
            let hidden = "*".repeat(chars.count().max(1));
            format!("{first}{hidden}@{domain}")
        }
        None => "*".repeat(email.chars().count()),
    }
}

pub struct UserCredentials {
    pub id: i32,
    /// As stored: the address the session answers with.
    pub email: String,
    pub password_hash: String,
    pub role: String,
    /// Whether the account is active. Use cases must reject login for
    /// inactive accounts — this field makes that invariant explicit at the
    /// domain model level rather than relying on a silent SQL filter.
    pub is_active: bool,
    pub locked_until: Option<DateTime<Utc>>,
}

pub struct NewUser {
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub full_name: Option<String>,
    /// The address is taken as the account's own from the start: an admin
    /// typed it for an account they create. A sign-up's is not, until the
    /// link sent to it is opened.
    pub email_verified: bool,
}

/// An account found by its email, for the "assign an account" forms.
pub struct ExistingUser {
    pub id: i32,
    pub role: String,
    /// Its owner opened a link sent to it (or an admin created it): only
    /// then may a record be handed to it, since anyone may sign up with a
    /// stranger's address.
    pub email_verified: bool,
}

pub struct UserAdmin {
    pub id: i32,
    pub email: String,
    pub full_name: Option<String>,
    pub role_code: String,
    pub role_name: String,
    pub is_active: bool,
    /// Created by the demo data at volume.
    pub is_demo: bool,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
    pub created_by_name: Option<String>,
    pub updated_by_name: Option<String>,
}

pub struct UserAdminDetail {
    pub id: i32,
    pub email: String,
    pub full_name: Option<String>,
    pub role_code: String,
    pub role_name: String,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
    pub email_verified: bool,
    /// The address a link sent lately would move the account to.
    pub pending_email: Option<String>,
}

/// The authenticated user's own profile — returned by GET /api/users/me.
pub struct UserMe {
    pub email: String,
    pub full_name: Option<String>,
    pub role: String,
    pub created_on: chrono::DateTime<chrono::Utc>,
    /// The link sent to `email` was opened.
    pub email_verified: bool,
    /// The address a link sent lately would move the account to; `email`
    /// stays the account's until it is opened.
    pub pending_email: Option<String>,
}

/// Minimal user info looked up by email for password-reset flows.
pub struct UserByEmail {
    pub id: i32,
    pub email: String,
}

/// A password-reset link emailed to an account.
pub struct NewResetToken {
    pub user_id: i32,
    /// SHA-256 of the emailed token; the raw value is never stored.
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}

/// A link that proves an address: the account's own (`new_email` none), or
/// the one it is to move to.
pub struct NewEmailVerification {
    pub user_id: i32,
    /// SHA-256 of the emailed token; the raw value is never stored.
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub new_email: Option<String>,
}

/// A confirmation link just opened.
pub struct ClaimedEmail {
    pub user_id: i32,
    /// The account's address now.
    pub email: String,
    /// The link moved the account to `email`.
    pub moved: bool,
    /// The address it moved from, when that one was confirmed: its owner is
    /// told of the move. An unconfirmed one may be a stranger's (a typo), who
    /// is told nothing.
    pub confirmed_previous: Option<String>,
}

/// Who a session belongs to and what they may do, loaded fresh on every
/// request so a revoked permission or a deactivated account takes effect at
/// once instead of when the session token expires.
pub struct ActorRecord {
    pub user_id: i32,
    /// `display_name`: the full name, else the email.
    pub name: String,
    pub role_code: String,
    pub is_active: bool,
    pub permissions: Vec<String>,
    /// Session tokens issued before this were ended (password changed).
    pub sessions_valid_after: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_is_shown_by_its_name_else_its_email() {
        assert_eq!(display_name(Some("Nguyen Van Tu"), "tu@gmail.com"), "Nguyen Van Tu");
        assert_eq!(display_name(Some("  "), "tu@gmail.com"), "tu@gmail.com");
        assert_eq!(display_name(None, "tu@gmail.com"), "tu@gmail.com");
    }

    #[test]
    fn a_masked_email_keeps_its_first_letter_and_domain() {
        assert_eq!(mask_email("tu.moi@gmail.com"), "t*****@gmail.com");
        assert_eq!(mask_email("a@x.vn"), "a*@x.vn");
    }
}
