//! Crop prices: the farm-gate price of a crop (a rice variety, a fruit) in
//! one area (a location preset) on one day, as a low–high range in đồng per
//! unit. One row per crop, area and day: entering the day's price again
//! replaces it. The crops themselves are reference data (seeded when the
//! table is empty, then edited by admins).

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_check, add_unique, by_user, flag, int, int_null, references, run, serial_id, stamped,
    varchar, varchar_null, CropPrices, Crops, Locations,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

fn crops() -> TableCreateStatement {
    Table::create()
        .table(Crops::Table)
        .col(serial_id(Crops::Id))
        .col(varchar(Crops::Name, 100).unique_key())
        .col(varchar(Crops::Kind, 20))
        .col(varchar(Crops::Unit, 20))
        .col(int(Crops::DisplayOrder).default(0))
        .col(flag(Crops::IsActive, true))
        .to_owned()
}

fn crop_prices() -> TableCreateStatement {
    Table::create()
        .table(CropPrices::Table)
        .col(serial_id(CropPrices::Id))
        .col(int(CropPrices::CropId))
        .col(int(CropPrices::LocationId))
        .col(ColumnDef::new(CropPrices::ObservedOn).date().not_null())
        .col(
            ColumnDef::new(CropPrices::PriceLow)
                .big_integer()
                .not_null(),
        )
        .col(
            ColumnDef::new(CropPrices::PriceHigh)
                .big_integer()
                .not_null(),
        )
        .col(varchar_null(CropPrices::Note, 200))
        .col(stamped(CropPrices::CreatedOn))
        .col(int_null(CropPrices::CreatedBy))
        .foreign_key(&mut references(
            CropPrices::Table,
            CropPrices::CropId,
            Crops::Table,
            Crops::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            CropPrices::Table,
            CropPrices::LocationId,
            Locations::Table,
            Locations::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut by_user(CropPrices::Table, CropPrices::CreatedBy))
        .to_owned()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(crops()).await?;
        manager.create_table(crop_prices()).await?;
        // Kinds frozen here; the live list is domain::constants::CROP_KINDS.
        run(
            manager,
            &[
                add_check(
                    &Crops::Table,
                    "chk_crops_kind",
                    Expr::col(Crops::Kind).is_in(["rice", "fruit"]),
                ),
                add_unique(
                    &CropPrices::Table,
                    "uq_crop_prices_day",
                    &[
                        &CropPrices::CropId,
                        &CropPrices::LocationId,
                        &CropPrices::ObservedOn,
                    ],
                ),
                add_check(
                    &CropPrices::Table,
                    "chk_crop_prices_range",
                    Expr::col(CropPrices::PriceLow)
                        .gt(0)
                        .and(Expr::col(CropPrices::PriceHigh).gte(Expr::col(CropPrices::PriceLow))),
                ),
            ],
        )
        .await?;
        manager
            .create_index(
                Index::create()
                    .name("ix_crop_prices_observed")
                    .table(CropPrices::Table)
                    .col((CropPrices::ObservedOn, IndexOrder::Desc))
                    .col((CropPrices::Id, IndexOrder::Desc))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(CropPrices::Table)
                    .table(Crops::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
