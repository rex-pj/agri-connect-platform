use async_trait::async_trait;

use crate::error::Error;

/// Repository for writing admin audit log entries.
///
/// Called after any destructive action (hide, deactivate, delete, unverify)
/// to persist who did what, to which entity, and why.
#[async_trait]
pub trait AuditLogRepositoryTrait {
    /// Append one audit entry. Failures are non-fatal — callers should
    /// log a warning but must not roll back the primary operation.
    async fn log(
        &self,
        admin_id: i32,
        action: &str,
        entity_type: &str,
        entity_id: i32,
        reason: Option<String>,
    ) -> Result<(), Error>;
}
