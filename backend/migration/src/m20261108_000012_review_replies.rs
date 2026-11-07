//! `reviews.reply`: the provider's public answer to a review, and when it was
//! last set. NULL: no answer.

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE reviews ADD COLUMN reply TEXT, ADD COLUMN replied_at TIMESTAMPTZ;",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE reviews DROP COLUMN IF EXISTS reply, DROP COLUMN IF EXISTS replied_at;",
            )
            .await?;
        Ok(())
    }
}
