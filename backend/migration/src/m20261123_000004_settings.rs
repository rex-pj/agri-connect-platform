//! Settings an admin changes without a deploy.
//!
//! - `platform_settings`: one row per setting moved off its default. The
//!   keys, their kinds, defaults and bounds are the registry in
//!   `application::settings`; a key with no row is at its default, so the
//!   table starts empty and a new setting needs no migration. `value` is the
//!   setting as text (a number, `true` / `false`, a phone, a link, an
//!   address); empty is a text setting turned off (no help line shown).
//! - `commodities.price_fresh_days`: how long a price of its crops stays on
//!   the board. Durian moves daily, rice weekly: one figure for every crop
//!   either hid good rice prices or showed stale durian ones. At most the
//!   window the board reads (`PRICE_HISTORY_DAYS`, 60).

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_check, by_user, create_updated_on_trigger, matches, run, stamped, varchar, Commodities,
    PlatformSettings,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Frozen here; the live rule is `application::settings::KEY_PATTERN`.
const KEY_PATTERN: &str = "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+$";
const KEY_LEN: u32 = 64;
const VALUE_LEN: u32 = 500;
/// Frozen here; the live figures are `application::constants`.
const PRICE_FRESH_DAYS_DEFAULT: i16 = 14;
const PRICE_FRESH_DAYS_MAX: i16 = 60;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlatformSettings::Table)
                    .col(varchar(PlatformSettings::Key, KEY_LEN).primary_key())
                    .col(varchar(PlatformSettings::Value, VALUE_LEN))
                    .col(stamped(PlatformSettings::UpdatedOn))
                    .col(ColumnDef::new(PlatformSettings::UpdatedBy).integer())
                    .foreign_key(&mut by_user(
                        PlatformSettings::Table,
                        PlatformSettings::UpdatedBy,
                    ))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Commodities::Table)
                    .add_column(
                        ColumnDef::new(Commodities::PriceFreshDays)
                            .small_integer()
                            .not_null()
                            .default(PRICE_FRESH_DAYS_DEFAULT),
                    )
                    .to_owned(),
            )
            .await?;
        run(
            manager,
            &[
                add_check(
                    &PlatformSettings::Table,
                    "chk_platform_settings_key",
                    matches(PlatformSettings::Key, KEY_PATTERN),
                ),
                add_check(
                    &Commodities::Table,
                    "chk_commodities_price_fresh_days",
                    Expr::col(Commodities::PriceFreshDays).between(1, PRICE_FRESH_DAYS_MAX),
                ),
                create_updated_on_trigger(&PlatformSettings::Table),
            ],
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Commodities::Table)
                    .drop_column(Commodities::PriceFreshDays)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(PlatformSettings::Table).to_owned())
            .await
    }
}
