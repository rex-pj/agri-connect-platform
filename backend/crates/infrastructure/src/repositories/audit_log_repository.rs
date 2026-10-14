use async_trait::async_trait;
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use std::sync::Arc;
use tracing::error;

use domain::repositories::audit_log_repository_trait::AuditLogRepositoryTrait;

use domain::Error;

use crate::entities::audit_log::{self, Entity as AuditLog};

pub struct AuditLogRepository {
    db: Arc<DatabaseConnection>,
}

impl AuditLogRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AuditLogRepositoryTrait for AuditLogRepository {
    async fn log(
        &self,
        admin_id: i32,
        action: &str,
        entity_type: &str,
        entity_id: i32,
        reason: Option<String>,
    ) -> Result<(), Error> {
        let model = audit_log::ActiveModel {
            admin_id: Set(Some(admin_id)),
            action: Set(action.to_string()),
            entity_type: Set(entity_type.to_string()),
            entity_id: Set(entity_id),
            reason: Set(reason),
            ..Default::default()
        };

        AuditLog::insert(model)
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("audit_log insert error: {e}");
                Error::internal("Failed to write audit log.")
            })
    }
}
