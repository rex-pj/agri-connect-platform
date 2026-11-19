use chrono::{DateTime, Utc};

/// One stored change to a record, as its page shows it: what was done
/// (`audit_action`'s code, e.g. `hide_service`), why, by whom and when.
pub struct AuditEntry {
    pub action: String,
    pub reason: Option<String>,
    /// The account that made it; `None` once that account is gone.
    pub actor: Option<String>,
    pub created_on: DateTime<Utc>,
}
