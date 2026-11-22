//! `inquiries.answered_on`: when the provider (or an admin) first marked a
//! contact request answered (`contacted` or `closed`). A review rests on an
//! answered request, and the provider decides which are; one answered
//! minutes after it was sent is a sign for the moderator
//! (`AdminReviewDto::suspect`). Requests answered before this column have
//! none.

use sea_orm_migration::prelude::*;

use crate::schema::{timestamp_null, Inquiries};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Inquiries::Table)
                    .add_column(timestamp_null(Inquiries::AnsweredOn))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Inquiries::Table)
                    .drop_column(Inquiries::AnsweredOn)
                    .to_owned(),
            )
            .await
    }
}
