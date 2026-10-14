use serde::{Deserialize, Serialize};

/// Database read model — represents a row in the `roles` table.
pub struct RoleModel {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
}

// ── Domain value object ───────────────────────────────────────────────────────

/// Core domain representation of a user's role.
///
/// Lives here (domain layer) so every layer can depend on it without
/// reaching into middleware or infrastructure. Serialisation derives allow
/// it to be embedded in JWT claims without pulling middleware types into the
/// application layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Admin,
    Provider,
}

impl UserRole {
    /// The DB role code string for this role. Matches constants `ROLE_ADMIN` / `ROLE_PROVIDER`.
    pub fn as_code(&self) -> &'static str {
        match self {
            UserRole::Admin => crate::constants::ROLE_ADMIN,
            UserRole::Provider => crate::constants::ROLE_PROVIDER,
        }
    }

    /// Parse a role code string into a `UserRole`. Returns `None` for
    /// unrecognised codes so callers can surface a domain-meaningful error.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            crate::constants::ROLE_ADMIN => Some(UserRole::Admin),
            crate::constants::ROLE_PROVIDER => Some(UserRole::Provider),
            _ => None,
        }
    }
}
