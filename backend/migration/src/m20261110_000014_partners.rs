//! Partners: the companies that pay for the demand report and sponsored
//! places. A contract covers one area (a location preset), some categories,
//! two dates and a monthly fee; it sponsors some providers of that area.

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_check, by_user, flag, int, int_null, is_vn_phone, references, run, serial_id, stamped,
    text_null, varchar, varchar_null, Businesses, Categories, Locations, PartnerContractCategories,
    PartnerContracts, PartnerSponsored, Partners, Users,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

fn partners() -> TableCreateStatement {
    Table::create()
        .table(Partners::Table)
        .col(serial_id(Partners::Id))
        .col(varchar(Partners::Name, 200))
        .col(varchar(Partners::Kind, 20))
        .col(varchar_null(Partners::ContactName, 200))
        .col(varchar_null(Partners::Phone, 20))
        .col(varchar_null(Partners::Email, 255))
        .col(int_null(Partners::AccountId).unique_key())
        .col(flag(Partners::IsActive, true))
        .col(stamped(Partners::CreatedOn))
        .col(stamped(Partners::UpdatedOn))
        .col(int_null(Partners::CreatedBy))
        .col(int_null(Partners::UpdatedBy))
        .foreign_key(&mut references(
            Partners::Table,
            Partners::AccountId,
            Users::Table,
            Users::Id,
            ForeignKeyAction::SetNull,
        ))
        .foreign_key(&mut by_user(Partners::Table, Partners::CreatedBy))
        .foreign_key(&mut by_user(Partners::Table, Partners::UpdatedBy))
        .to_owned()
}

fn partner_contracts() -> TableCreateStatement {
    Table::create()
        .table(PartnerContracts::Table)
        .col(serial_id(PartnerContracts::Id))
        .col(int(PartnerContracts::PartnerId))
        .col(int(PartnerContracts::LocationId))
        .col(ColumnDef::new(PartnerContracts::StartsOn).date().not_null())
        .col(ColumnDef::new(PartnerContracts::EndsOn).date().not_null())
        .col(
            ColumnDef::new(PartnerContracts::MonthlyFee)
                .big_integer()
                .not_null(),
        )
        .col(text_null(PartnerContracts::Note))
        .col(stamped(PartnerContracts::CreatedOn))
        .col(int_null(PartnerContracts::CreatedBy))
        .foreign_key(&mut references(
            PartnerContracts::Table,
            PartnerContracts::PartnerId,
            Partners::Table,
            Partners::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            PartnerContracts::Table,
            PartnerContracts::LocationId,
            Locations::Table,
            Locations::Id,
            ForeignKeyAction::Restrict,
        ))
        .foreign_key(&mut by_user(
            PartnerContracts::Table,
            PartnerContracts::CreatedBy,
        ))
        .to_owned()
}

fn partner_contract_categories() -> TableCreateStatement {
    Table::create()
        .table(PartnerContractCategories::Table)
        .col(int(PartnerContractCategories::ContractId))
        .col(int(PartnerContractCategories::CategoryId))
        .primary_key(
            Index::create()
                .col(PartnerContractCategories::ContractId)
                .col(PartnerContractCategories::CategoryId),
        )
        .foreign_key(&mut references(
            PartnerContractCategories::Table,
            PartnerContractCategories::ContractId,
            PartnerContracts::Table,
            PartnerContracts::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            PartnerContractCategories::Table,
            PartnerContractCategories::CategoryId,
            Categories::Table,
            Categories::Id,
            ForeignKeyAction::Cascade,
        ))
        .to_owned()
}

fn partner_sponsored() -> TableCreateStatement {
    Table::create()
        .table(PartnerSponsored::Table)
        .col(int(PartnerSponsored::ContractId))
        .col(int(PartnerSponsored::BusinessId))
        .primary_key(
            Index::create()
                .col(PartnerSponsored::ContractId)
                .col(PartnerSponsored::BusinessId),
        )
        .foreign_key(&mut references(
            PartnerSponsored::Table,
            PartnerSponsored::ContractId,
            PartnerContracts::Table,
            PartnerContracts::Id,
            ForeignKeyAction::Cascade,
        ))
        .foreign_key(&mut references(
            PartnerSponsored::Table,
            PartnerSponsored::BusinessId,
            Businesses::Table,
            Businesses::Id,
            ForeignKeyAction::Cascade,
        ))
        .to_owned()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(partners()).await?;
        manager.create_table(partner_contracts()).await?;
        // Kinds frozen here; the live list is domain::constants::PARTNER_KINDS.
        run(
            manager,
            &[
                add_check(
                    &Partners::Table,
                    "chk_partners_kind",
                    Expr::col(Partners::Kind).is_in(["brand", "dealer", "cooperative", "exporter"]),
                ),
                add_check(
                    &Partners::Table,
                    "chk_partners_phone",
                    Expr::col(Partners::Phone)
                        .is_null()
                        .or(is_vn_phone(Partners::Phone)),
                ),
                add_check(
                    &PartnerContracts::Table,
                    "chk_partner_contracts_dates",
                    Expr::col(PartnerContracts::EndsOn).gte(Expr::col(PartnerContracts::StartsOn)),
                ),
                add_check(
                    &PartnerContracts::Table,
                    "chk_partner_contracts_fee",
                    Expr::col(PartnerContracts::MonthlyFee).gte(0),
                ),
            ],
        )
        .await?;
        manager
            .create_index(
                Index::create()
                    .name("ix_partner_contracts_partner")
                    .table(PartnerContracts::Table)
                    .col(PartnerContracts::PartnerId)
                    .to_owned(),
            )
            .await?;
        manager.create_table(partner_contract_categories()).await?;
        manager.create_table(partner_sponsored()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PartnerSponsored::Table)
                    .table(PartnerContractCategories::Table)
                    .table(PartnerContracts::Table)
                    .table(Partners::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
