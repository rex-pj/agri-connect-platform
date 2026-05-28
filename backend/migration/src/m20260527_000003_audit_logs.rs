use sea_orm_migration::prelude::*;

use crate::enums::user::User;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum AuditLog {
    #[iden = "audit_logs"]
    Table,
    Id,
    AdminId,
    Action,
    EntityType,
    EntityId,
    Reason,
    CreatedOn,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        use sea_orm::ConnectionTrait;
        let db = manager.get_connection();

        manager
            .create_table(
                Table::create()
                    .table(AuditLog::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuditLog::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // Nullable so audit logs survive admin account deletion.
                    .col(ColumnDef::new(AuditLog::AdminId).integer().null())
                    .col(ColumnDef::new(AuditLog::Action).string_len(100).not_null())
                    .col(ColumnDef::new(AuditLog::EntityType).string_len(50).not_null())
                    .col(ColumnDef::new(AuditLog::EntityId).integer().not_null())
                    .col(ColumnDef::new(AuditLog::Reason).text().null())
                    .col(
                        ColumnDef::new(AuditLog::CreatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_audit_logs_admin_id")
                            .from(AuditLog::Table, AuditLog::AdminId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS ix_audit_logs_admin_id \
             ON audit_logs(admin_id)",
        )
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS ix_audit_logs_entity \
             ON audit_logs(entity_type, entity_id)",
        )
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS ix_audit_logs_created_on \
             ON audit_logs(created_on DESC)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditLog::Table).to_owned())
            .await
    }
}
