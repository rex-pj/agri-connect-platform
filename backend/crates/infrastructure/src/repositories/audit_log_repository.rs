use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, JoinType, QueryFilter,
    QueryOrder, QuerySelect, RelationTrait, Set,
};
use std::sync::Arc;
use tracing::error;

use domain::{
    models::audit::AuditEntry, repositories::audit_log_repository_trait::AuditLogRepositoryTrait,
};

use domain::Error;

use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        audit_log::{self, Entity as AuditLog},
        user,
    },
    sql::{account_name, QueryAs},
};

pub struct AuditLogRepository {
    db: Arc<DatabaseConnection>,
}

impl AuditLogRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[derive(FromQueryResult)]
struct AuditEntryRow {
    action: String,
    reason: Option<String>,
    account_name: Option<String>,
    created_on: DateTimeWithTimeZone,
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

    async fn recent(
        &self,
        entity_type: &str,
        entity_id: i32,
        limit: u64,
    ) -> Result<Vec<AuditEntry>, Error> {
        AuditLog::find()
            .select_only()
            .column(audit_log::Column::Action)
            .column(audit_log::Column::Reason)
            .expr_as(account_name(user::Entity), QueryAs::AccountName)
            .column(audit_log::Column::CreatedOn)
            .join(JoinType::LeftJoin, audit_log::Relation::Admin.def())
            .filter(audit_log::Column::EntityType.eq(entity_type))
            .filter(audit_log::Column::EntityId.eq(entity_id))
            .order_by_desc(audit_log::Column::Id)
            .limit(limit)
            .into_model::<AuditEntryRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| AuditEntry {
                        action: r.action,
                        reason: r.reason,
                        actor: r.account_name,
                        created_on: r.created_on.with_timezone(&Utc),
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("audit_log recent error ({entity_type} {entity_id}): {e}");
                Error::internal("Failed to load the history.")
            })
    }
}
