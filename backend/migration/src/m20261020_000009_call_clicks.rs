//! A tap on a public call button: the click-to-call KPI, and the leads a
//! provider can see on their dashboard. Only what was called and when; never
//! who called (no address, no phone, no session).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum CallClicks {
    Table,
    Id,
    BusinessId,
    ServiceId,
    ProductId,
    CreatedOn,
}

#[derive(DeriveIden)]
enum Businesses {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Services {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Products {
    Table,
    Id,
}

const INDEX: &str = "ix_call_clicks_business_created";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CallClicks::Table)
                    .col(
                        ColumnDef::new(CallClicks::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CallClicks::BusinessId).integer().not_null())
                    .col(ColumnDef::new(CallClicks::ServiceId).integer().null())
                    .col(ColumnDef::new(CallClicks::ProductId).integer().null())
                    .col(
                        ColumnDef::new(CallClicks::CreatedOn)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CallClicks::Table, CallClicks::BusinessId)
                            .to(Businesses::Table, Businesses::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CallClicks::Table, CallClicks::ServiceId)
                            .to(Services::Table, Services::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CallClicks::Table, CallClicks::ProductId)
                            .to(Products::Table, Products::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        // Counts are per business over a recent window.
        manager
            .create_index(
                Index::create()
                    .name(INDEX)
                    .table(CallClicks::Table)
                    .col(CallClicks::BusinessId)
                    .col(CallClicks::CreatedOn)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(CallClicks::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
