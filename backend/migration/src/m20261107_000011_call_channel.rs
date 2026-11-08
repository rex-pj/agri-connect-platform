//! `call_clicks.channel`: how the farmer reached out, a phone call or a Zalo
//! chat (both count as a lead). Rows from before it were all calls.

use sea_orm_migration::prelude::*;

use crate::schema::{add_check, run, varchar, CallClicks};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CallClicks::Table)
                    .add_column(varchar(CallClicks::Channel, 10).default("call"))
                    .to_owned(),
            )
            .await?;
        // Frozen here; the live list is domain::constants::CALL_CHANNELS.
        run(
            manager,
            &[add_check(
                &CallClicks::Table,
                "chk_call_clicks_channel",
                Expr::col(CallClicks::Channel).is_in(["call", "zalo"]),
            )],
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CallClicks::Table)
                    .drop_column(CallClicks::Channel)
                    .to_owned(),
            )
            .await
    }
}
