//! `users.sessions_valid_after`: access tokens issued before it are refused.
//! A password change or reset sets it, so other devices are signed out at
//! once instead of when their 1-hour access token runs out. NULL: no cut-off.

use sea_orm_migration::prelude::*;

use crate::schema::{timestamp_null, Users};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(timestamp_null(Users::SessionsValidAfter))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::SessionsValidAfter)
                    .to_owned(),
            )
            .await
    }
}
