//! Vietnam's administrative units since 1 July 2025: 34 provinces and their
//! 3 321 wards, communes and special zones, with official codes, a point
//! inside each, and each ward's boundary. The rows themselves come from the
//! bundled dataset, loaded at startup (`AdminUnitRepository::sync`); this
//! only makes room for them.
//!
//! The demand report counts a lead under the ward its business lies in (and
//! that ward's province), so searches keep the ward they were made in, and a
//! partner's contract covers a province or one ward of it. The older
//! `location_id` columns stay for the rows written before: the first load
//! gives them the ward of their preset's point.

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_check, flag, references, run, varchar, varchar_null, GeographyMultiPolygon, GeographyPoint,
    IndexMethod, PartnerContracts, SearchEvents,
};

/// An official unit code: two digits for a province, five for a ward.
const CODE_LEN: u32 = 5;

#[derive(DeriveIden, Clone, Copy)]
enum Provinces {
    Table,
    Code,
    Name,
    FullName,
    UnitType,
    Center,
    IsActive,
}

#[derive(DeriveIden, Clone, Copy)]
enum Wards {
    Table,
    Code,
    ProvinceCode,
    Name,
    FullName,
    UnitType,
    Center,
    Boundary,
    IsActive,
}

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Provinces::Table)
                    .col(varchar(Provinces::Code, CODE_LEN).primary_key())
                    .col(varchar(Provinces::Name, 100))
                    .col(varchar(Provinces::FullName, 120))
                    .col(varchar(Provinces::UnitType, 20))
                    .col(
                        ColumnDef::new(Provinces::Center)
                            .custom(GeographyPoint)
                            .not_null(),
                    )
                    .col(flag(Provinces::IsActive, true))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Wards::Table)
                    .col(varchar(Wards::Code, CODE_LEN).primary_key())
                    .col(varchar(Wards::ProvinceCode, CODE_LEN))
                    .col(varchar(Wards::Name, 100))
                    .col(varchar(Wards::FullName, 120))
                    .col(varchar(Wards::UnitType, 20))
                    .col(
                        ColumnDef::new(Wards::Center)
                            .custom(GeographyPoint)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Wards::Boundary)
                            .custom(GeographyMultiPolygon)
                            .not_null(),
                    )
                    .col(flag(Wards::IsActive, true))
                    .foreign_key(&mut references(
                        Wards::Table,
                        Wards::ProvinceCode,
                        Provinces::Table,
                        Provinces::Code,
                        ForeignKeyAction::Restrict,
                    ))
                    .to_owned(),
            )
            .await?;
        // Unit types frozen here; the dataset's are province, city, ward,
        // commune and special_zone.
        run(
            manager,
            &[
                add_check(
                    &Provinces::Table,
                    "chk_provinces_unit_type",
                    Expr::col(Provinces::UnitType).is_in(["province", "city"]),
                ),
                add_check(
                    &Wards::Table,
                    "chk_wards_unit_type",
                    Expr::col(Wards::UnitType).is_in(["ward", "commune", "special_zone"]),
                ),
            ],
        )
        .await?;
        for index in [
            Index::create()
                .name("ix_wards_province")
                .table(Wards::Table)
                .col(Wards::ProvinceCode)
                .to_owned(),
            Index::create()
                .name("ix_wards_boundary")
                .table(Wards::Table)
                .index_type(IndexType::Custom(IndexMethod::Gist.into_iden()))
                .col(Wards::Boundary)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(SearchEvents::Table)
                    .add_column(varchar_null(SearchEvents::WardCode, CODE_LEN))
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(references(
                SearchEvents::Table,
                SearchEvents::WardCode,
                Wards::Table,
                Wards::Code,
                ForeignKeyAction::SetNull,
            ))
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(PartnerContracts::Table)
                    .add_column(varchar_null(PartnerContracts::ProvinceCode, CODE_LEN))
                    .add_column(varchar_null(PartnerContracts::WardCode, CODE_LEN))
                    .modify_column(
                        ColumnDef::new(PartnerContracts::LocationId)
                            .integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        for (column, to, to_id) in [
            (
                PartnerContracts::ProvinceCode,
                Provinces::Table.into_iden(),
                Provinces::Code.into_iden(),
            ),
            (
                PartnerContracts::WardCode,
                Wards::Table.into_iden(),
                Wards::Code.into_iden(),
            ),
        ] {
            manager
                .create_foreign_key(references(
                    PartnerContracts::Table,
                    column,
                    to,
                    to_id,
                    ForeignKeyAction::Restrict,
                ))
                .await?;
        }
        // A contract has an area: its province (and maybe one ward of it),
        // or, until the first load fills that in, its older preset.
        run(
            manager,
            &[
                add_check(
                    &PartnerContracts::Table,
                    "chk_partner_contracts_area",
                    Expr::col(PartnerContracts::ProvinceCode)
                        .is_not_null()
                        .or(Expr::col(PartnerContracts::LocationId).is_not_null()),
                ),
                add_check(
                    &PartnerContracts::Table,
                    "chk_partner_contracts_ward",
                    Expr::col(PartnerContracts::WardCode)
                        .is_null()
                        .or(Expr::col(PartnerContracts::ProvinceCode).is_not_null()),
                ),
            ],
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Contracts made since have no preset to fall back on: they go.
        manager
            .exec_stmt(
                Query::delete()
                    .from_table(PartnerContracts::Table)
                    .and_where(Expr::col(PartnerContracts::LocationId).is_null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PartnerContracts::Table)
                    .drop_column(PartnerContracts::ProvinceCode)
                    .drop_column(PartnerContracts::WardCode)
                    .modify_column(
                        ColumnDef::new(PartnerContracts::LocationId)
                            .integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(SearchEvents::Table)
                    .drop_column(SearchEvents::WardCode)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Wards::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Provinces::Table).if_exists().to_owned())
            .await
    }
}
