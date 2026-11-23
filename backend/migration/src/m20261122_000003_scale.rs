//! What the schema needs once the tables are large.
//!
//! - `businesses.ward_code`: the ward a business's position lies in, kept
//!   with the position. The demand report reads it instead of asking, for
//!   every business on every load, which boundary covers its point.
//! - `call_click_weeks`, `search_event_weeks`: taps and searches older than
//!   any report reads, counted by week. The rows they were counted from are
//!   deleted, so `call_clicks` and `search_events` stay the size of a few
//!   months however long the platform runs.
//! - `ix_call_clicks_created_brin`: the report and the dashboard read taps
//!   by time alone. The table is written in time order, so a block-range
//!   index of a few pages answers it and costs an insert almost nothing.
//! - Four indexes nothing needs are dropped, each a write on every row
//!   stored: `idx_services_live` and `idx_products_live` (the same column as
//!   `idx_services_business` / `idx_products_business`, which serve the
//!   owner's lists and the foreign key too), `idx_businesses_active` (two
//!   booleans: nearly every row has the value asked for) and
//!   `idx_inquiries_status` (a status is always asked for within one
//!   business, which `idx_inquiries_business` finds).

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_check, at_most_one, big_serial_id, flag, int, int_null, references, run, varchar,
    varchar_null, Businesses, CallClickWeeks, CallClicks, IndexMethod, Inquiries, OfferTypes,
    Products, SearchEventWeeks, ServiceTypes, Services, SupplyTypes, Wards,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Frozen here; the live list is in `domain::constants`.
const CALL_CHANNELS: [&str; 2] = ["call", "zalo"];
/// An official unit code: 5 digits for a ward.
const UNIT_CODE_LEN: u32 = 5;
const CHANNEL_LEN: u32 = 10;

const BUSINESS_WARD_FK: &str = "businesses_ward_code_fkey";

/// `is_active = TRUE AND deleted_at IS NULL`: the live rows.
fn live<A: IntoIden + 'static, D: IntoIden + 'static>(is_active: A, deleted_at: D) -> Condition {
    Condition::all()
        .add(Expr::col(is_active).eq(true))
        .add(Expr::col(deleted_at).is_null())
}

fn call_click_weeks() -> TableCreateStatement {
    Table::create()
        .table(CallClickWeeks::Table)
        .col(big_serial_id(CallClickWeeks::Id))
        .col(
            ColumnDef::new(CallClickWeeks::WeekStartsOn)
                .date()
                .not_null(),
        )
        .col(int(CallClickWeeks::BusinessId))
        .col(int_null(CallClickWeeks::ServiceTypeId))
        .col(int_null(CallClickWeeks::SupplyTypeId))
        .col(varchar(CallClickWeeks::Channel, CHANNEL_LEN))
        .col(int(CallClickWeeks::Taps))
        .foreign_key(&mut references(
            CallClickWeeks::Table,
            CallClickWeeks::BusinessId,
            Businesses::Table,
            Businesses::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            CallClickWeeks::Table,
            CallClickWeeks::ServiceTypeId,
            ServiceTypes::Table,
            OfferTypes::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut references(
            CallClickWeeks::Table,
            CallClickWeeks::SupplyTypeId,
            SupplyTypes::Table,
            OfferTypes::Id,
            ForeignKeyAction::SetNull,
        ))
        .to_owned()
}

fn search_event_weeks() -> TableCreateStatement {
    Table::create()
        .table(SearchEventWeeks::Table)
        .col(big_serial_id(SearchEventWeeks::Id))
        .col(
            ColumnDef::new(SearchEventWeeks::WeekStartsOn)
                .date()
                .not_null(),
        )
        .col(int_null(SearchEventWeeks::ServiceTypeId))
        .col(int_null(SearchEventWeeks::SupplyTypeId))
        .col(varchar_null(SearchEventWeeks::WardCode, UNIT_CODE_LEN))
        .col(int(SearchEventWeeks::Searches))
        .col(int(SearchEventWeeks::Unmet))
        .col(flag(SearchEventWeeks::IsDemo, false))
        .foreign_key(&mut references(
            SearchEventWeeks::Table,
            SearchEventWeeks::ServiceTypeId,
            ServiceTypes::Table,
            OfferTypes::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut references(
            SearchEventWeeks::Table,
            SearchEventWeeks::SupplyTypeId,
            SupplyTypes::Table,
            OfferTypes::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut references(
            SearchEventWeeks::Table,
            SearchEventWeeks::WardCode,
            Wards::Table,
            Wards::Code,
            ForeignKeyAction::SetNull,
        ))
        .to_owned()
}

fn checks() -> Vec<String> {
    vec![
        add_check(
            &CallClickWeeks::Table,
            "chk_call_click_weeks_one_type",
            at_most_one(CallClickWeeks::ServiceTypeId, CallClickWeeks::SupplyTypeId),
        ),
        add_check(
            &CallClickWeeks::Table,
            "chk_call_click_weeks_channel",
            Expr::col(CallClickWeeks::Channel).is_in(CALL_CHANNELS),
        ),
        add_check(
            &CallClickWeeks::Table,
            "chk_call_click_weeks_taps",
            Expr::col(CallClickWeeks::Taps).gt(0),
        ),
        add_check(
            &SearchEventWeeks::Table,
            "chk_search_event_weeks_one_type",
            at_most_one(
                SearchEventWeeks::ServiceTypeId,
                SearchEventWeeks::SupplyTypeId,
            ),
        ),
        // Some of a week's searches found nothing, never more than were made.
        add_check(
            &SearchEventWeeks::Table,
            "chk_search_event_weeks_counts",
            Expr::col(SearchEventWeeks::Searches)
                .gt(0)
                .and(Expr::col(SearchEventWeeks::Unmet).gte(0))
                .and(Expr::col(SearchEventWeeks::Unmet).lte(Expr::col(SearchEventWeeks::Searches))),
        ),
    ]
}

fn index<T: IntoIden + 'static>(name: &str, table: T) -> IndexCreateStatement {
    Index::create().name(name).table(table).to_owned()
}

/// The indexes this migration adds.
fn added() -> Vec<IndexCreateStatement> {
    vec![
        index("ix_call_clicks_created_brin", CallClicks::Table)
            .index_type(IndexType::Custom(IndexMethod::Brin.into_iden()))
            .col(CallClicks::CreatedOn)
            .to_owned(),
        index("ix_call_click_weeks_week", CallClickWeeks::Table)
            .col(CallClickWeeks::WeekStartsOn)
            .to_owned(),
        // Deleting a business deletes its weeks.
        index("ix_call_click_weeks_business", CallClickWeeks::Table)
            .col(CallClickWeeks::BusinessId)
            .to_owned(),
        index("ix_search_event_weeks_week", SearchEventWeeks::Table)
            .col(SearchEventWeeks::WeekStartsOn)
            .to_owned(),
    ]
}

/// The indexes this migration drops, as the baseline made them.
fn dropped() -> Vec<IndexCreateStatement> {
    vec![
        index("idx_services_live", Services::Table)
            .col(Services::BusinessId)
            .cond_where(live(Services::IsActive, Services::DeletedAt))
            .to_owned(),
        index("idx_products_live", Products::Table)
            .col(Products::BusinessId)
            .cond_where(live(Products::IsActive, Products::DeletedAt))
            .to_owned(),
        index("idx_businesses_active", Businesses::Table)
            .col(Businesses::IsActive)
            .col(Businesses::IsVerified)
            .and_where(Expr::col(Businesses::DeletedAt).is_null())
            .to_owned(),
        index("idx_inquiries_status", Inquiries::Table)
            .col(Inquiries::Status)
            .to_owned(),
    ]
}

/// The names of the indexes `dropped` makes.
const DROPPED: [&str; 4] = [
    "idx_services_live",
    "idx_products_live",
    "idx_businesses_active",
    "idx_inquiries_status",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Businesses::Table)
                    .add_column(varchar_null(Businesses::WardCode, UNIT_CODE_LEN))
                    // A ward is turned off, never deleted.
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name(BUSINESS_WARD_FK)
                            .from_tbl(Businesses::Table)
                            .from_col(Businesses::WardCode)
                            .to_tbl(Wards::Table)
                            .to_col(Wards::Code),
                    )
                    .to_owned(),
            )
            .await?;
        manager.create_table(call_click_weeks()).await?;
        manager.create_table(search_event_weeks()).await?;
        run(manager, &checks()).await?;
        for index in added() {
            manager.create_index(index).await?;
        }
        for name in DROPPED {
            manager
                .drop_index(Index::drop().name(name).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for index in dropped() {
            manager.create_index(index).await?;
        }
        manager
            .drop_index(
                Index::drop()
                    .name("ix_call_clicks_created_brin")
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(SearchEventWeeks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CallClickWeeks::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Businesses::Table)
                    // Its reference to the wards goes with the column.
                    .drop_column(Businesses::WardCode)
                    .to_owned(),
            )
            .await
    }
}
