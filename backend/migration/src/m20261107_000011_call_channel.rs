//! `call_clicks.channel`: how the farmer reached out, a phone call or a Zalo
//! chat (both count as a lead). Rows from before it were all calls.

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Keep in sync with domain::constants::CALL_CHANNELS.
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE call_clicks \
                 ADD COLUMN channel VARCHAR(10) NOT NULL DEFAULT 'call', \
                 ADD CONSTRAINT chk_call_clicks_channel CHECK (channel IN ('call', 'zalo'));",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE call_clicks DROP COLUMN IF EXISTS channel;")
            .await?;
        Ok(())
    }
}
