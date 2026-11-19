use async_trait::async_trait;

use crate::{error::Error, models::audit::AuditEntry};

/// Repository for the admin audit log: written after every change, read
/// back by a record's page (who changed it, when, why).
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

    /// The latest `limit` entries about one record, newest first.
    async fn recent(
        &self,
        entity_type: &str,
        entity_id: i32,
        limit: u64,
    ) -> Result<Vec<AuditEntry>, Error>;
}
