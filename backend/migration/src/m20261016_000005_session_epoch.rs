//! `users.sessions_valid_after`: access tokens issued before it are refused.
//! A password change or reset sets it, so other devices are signed out at
//! once instead of when their 1-hour access token runs out. NULL: no cut-off.

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE users ADD COLUMN sessions_valid_after TIMESTAMPTZ;")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE users DROP COLUMN IF EXISTS sessions_valid_after;")
            .await?;
        Ok(())
    }
}
