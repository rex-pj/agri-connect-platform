//! `reviews.reply`: the provider's public answer to a review, and when it was
//! last set. NULL: no answer.

use sea_orm_migration::prelude::*;

use crate::schema::{text_null, timestamp_null, Reviews};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Reviews::Table)
                    .add_column(text_null(Reviews::Reply))
                    .add_column(timestamp_null(Reviews::RepliedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Reviews::Table)
                    .drop_column(Reviews::Reply)
                    .drop_column(Reviews::RepliedAt)
                    .to_owned(),
            )
            .await
    }
}
