use sea_orm::entity::prelude::*;

/// Sea-ORM entity for the `audit_logs` table: who changed what, and why.
/// Appended by the audit subscriber, read back by a record's admin page.
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
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::AdminId",
        to = "super::user::Column::Id"
    )]
    Admin,
}

impl ActiveModelBehavior for ActiveModel {}
