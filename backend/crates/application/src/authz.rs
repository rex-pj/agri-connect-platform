//! Authorization: who may do what, decided by permission key, never by role
//! name. Use cases call `actor.require(KEY)?` before doing anything; pages use
//! `authorize` to compute their `can_*` flags with the same rule.

use std::collections::BTreeSet;

use domain::{models::user::ActorRecord, Error, ErrorCode, Result};

/// An authenticated, active user and their effective permissions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub user_id: i32,
    /// What pages call the account: its full name, else its email.
    pub name: String,
    /// For display only (e.g. a badge). Access is never decided on it.
    pub role_code: String,
    permissions: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Allow,
    Unauthenticated,
    Forbidden,
}

/// The one authorization rule, pure so it can be tested without an AppState.
pub fn authorize(actor: Option<&Actor>, key: &str) -> Disposition {
    match actor {
        None => Disposition::Unauthenticated,
        Some(a) if a.permissions.contains(key) => Disposition::Allow,
        Some(_) => Disposition::Forbidden,
    }
}

impl Actor {
    /// `None` for a deactivated account: its still-valid token buys nothing.
    pub fn from_record(record: ActorRecord) -> Option<Self> {
        record.is_active.then(|| Self {
            user_id: record.user_id,
            name: record.name,
            role_code: record.role_code,
            permissions: record.permissions.into_iter().collect(),
        })
    }

    pub fn can(&self, key: &str) -> bool {
        authorize(Some(self), key) == Disposition::Allow
    }

    pub fn require(&self, key: &str) -> Result<()> {
        match authorize(Some(self), key) {
            Disposition::Allow => Ok(()),
            Disposition::Unauthenticated => Err(Error::new(ErrorCode::AuthenticationRequired)),
            Disposition::Forbidden => {
                Err(Error::new(ErrorCode::PermissionDenied).with_detail(format!("missing {key}")))
            }
        }
    }

    pub fn permissions(&self) -> impl Iterator<Item = &str> {
        self.permissions.iter().map(String::as_str)
    }
}

/// For handlers: an anonymous caller of an endpoint that needs a session.
pub fn require_actor(actor: Option<Actor>) -> Result<Actor> {
    actor.ok_or_else(|| Error::new(ErrorCode::AuthenticationRequired))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(perms: &[&str], active: bool) -> Option<Actor> {
        Actor::from_record(ActorRecord {
            user_id: 1,
            name: "u".into(),
            role_code: "admin".into(),
            is_active: active,
            permissions: perms.iter().map(|p| (*p).to_owned()).collect(),
            sessions_valid_after: None,
        })
    }

    #[test]
    fn decisions() {
        let a = actor(&["provider.verify"], true);
        assert_eq!(
            authorize(None, "provider.verify"),
            Disposition::Unauthenticated
        );
        assert_eq!(authorize(a.as_ref(), "provider.verify"), Disposition::Allow);
        assert_eq!(authorize(a.as_ref(), "user.read"), Disposition::Forbidden);
    }

    #[test]
    fn a_role_name_grants_nothing_by_itself() {
        // role_code is "admin", but without the key the answer is no.
        let a = actor(&[], true);
        assert_eq!(
            a.map(|a| a.require("user.read").err().map(|e| e.code())),
            Some(Some(ErrorCode::PermissionDenied))
        );
    }

    #[test]
    fn a_deactivated_account_is_no_actor() {
        assert!(actor(&["user.read"], false).is_none());
    }
}
