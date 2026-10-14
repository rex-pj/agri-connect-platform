use sea_orm::entity::prelude::*;

/// Sea-ORM entity for the `audit_logs` table.
/// Write-only: used by the audit trail to record admin destructive actions.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "audit_logs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub admin_id: Option<i32>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: i32,
    pub reason: Option<String>,
    pub created_on: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
