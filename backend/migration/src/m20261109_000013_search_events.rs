//! `search_events`: what farmers looked for, for the demand report. Only the
//! words (lowercased, capped), the category filter, the area and how many
//! results came back; never who searched (no address, no session).

use sea_orm_migration::prelude::*;

use crate::schema::{
    big_serial_id, int, int_null, references, stamped, varchar_null, Categories, Locations,
    SearchEvents,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const INDEX: &str = "ix_search_events_created_on";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SearchEvents::Table)
                    .col(big_serial_id(SearchEvents::Id))
                    .col(varchar_null(SearchEvents::Query, 100))
                    .col(int_null(SearchEvents::CategoryId))
                    .col(int_null(SearchEvents::LocationId))
                    .col(int(SearchEvents::Results))
                    .col(stamped(SearchEvents::CreatedOn))
                    .foreign_key(&mut references(
                        SearchEvents::Table,
                        SearchEvents::CategoryId,
                        Categories::Table,
                        Categories::Id,
                        ForeignKeyAction::SetNull,
                    ))
                    .foreign_key(&mut references(
                        SearchEvents::Table,
                        SearchEvents::LocationId,
                        Locations::Table,
                        Locations::Id,
                        ForeignKeyAction::SetNull,
                    ))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name(INDEX)
                    .table(SearchEvents::Table)
                    .col(SearchEvents::CreatedOn)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(SearchEvents::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
