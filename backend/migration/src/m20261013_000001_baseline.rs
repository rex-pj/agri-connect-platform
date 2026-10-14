use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("m20261013_000001_baseline.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DROP TABLE IF EXISTS audit_logs, locations, reviews, inquiries, products, \
                 services, businesses, categories, catalogue_state, role_permissions, \
                 permissions, password_reset_tokens, users, roles CASCADE; \
                 DROP FUNCTION IF EXISTS fn_set_updated_on CASCADE;",
            )
            .await?;
        Ok(())
    }
}
