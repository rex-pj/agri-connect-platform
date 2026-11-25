//! Indexes on the columns that name who created or last changed a row.
//!
//! Deleting an account sets each of them to NULL where it names it (`SET
//! NULL`), and PostgreSQL looks for those rows once per account per column.
//! Without an index each look reads the whole table: removing the 4 967
//! accounts of the demo data at volume spent a minute in these checks and
//! hit the statement timeout (500 accounts: 6.3 s, `inquiries.updated_by`
//! alone 0.6 s). With these it takes 2 s. Only the demo data deletes
//! accounts (real ones are soft-deleted), so only the tables that grow with
//! it are indexed; the catalogue's and the settings' few rows read fast
//! enough. Partial (`IS NOT NULL`): a row nobody touched costs no entry.

use sea_orm_migration::prelude::*;

use crate::schema::{
    Alerts, Businesses, CropPrices, Inquiries, PartnerContracts, Partners, Products, Reviews,
    Services, Users,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// One partial index on a column naming an account, with its name.
fn by_user_index<T, C>(
    name: &'static str,
    table: T,
    column: C,
) -> (&'static str, IndexCreateStatement)
where
    T: IntoIden + 'static,
    C: IntoIden + Clone + 'static,
{
    let index = Index::create()
        .name(name)
        .table(table)
        .col(column.clone())
        .and_where(Expr::col(column).is_not_null())
        .to_owned();
    (name, index)
}

fn indexes() -> Vec<(&'static str, IndexCreateStatement)> {
    vec![
        by_user_index("ix_users_created_by", Users::Table, Users::CreatedBy),
        by_user_index("ix_users_updated_by", Users::Table, Users::UpdatedBy),
        by_user_index(
            "ix_businesses_created_by",
            Businesses::Table,
            Businesses::CreatedBy,
        ),
        by_user_index(
            "ix_businesses_updated_by",
            Businesses::Table,
            Businesses::UpdatedBy,
        ),
        by_user_index(
            "ix_services_created_by",
            Services::Table,
            Services::CreatedBy,
        ),
        by_user_index(
            "ix_services_updated_by",
            Services::Table,
            Services::UpdatedBy,
        ),
        by_user_index(
            "ix_products_created_by",
            Products::Table,
            Products::CreatedBy,
        ),
        by_user_index(
            "ix_products_updated_by",
            Products::Table,
            Products::UpdatedBy,
        ),
        by_user_index(
            "ix_inquiries_updated_by",
            Inquiries::Table,
            Inquiries::UpdatedBy,
        ),
        by_user_index("ix_reviews_updated_by", Reviews::Table, Reviews::UpdatedBy),
        by_user_index(
            "ix_crop_prices_created_by",
            CropPrices::Table,
            CropPrices::CreatedBy,
        ),
        by_user_index("ix_alerts_created_by", Alerts::Table, Alerts::CreatedBy),
        by_user_index("ix_alerts_updated_by", Alerts::Table, Alerts::UpdatedBy),
        by_user_index(
            "ix_partners_created_by",
            Partners::Table,
            Partners::CreatedBy,
        ),
        by_user_index(
            "ix_partners_updated_by",
            Partners::Table,
            Partners::UpdatedBy,
        ),
        by_user_index(
            "ix_partner_contracts_created_by",
            PartnerContracts::Table,
            PartnerContracts::CreatedBy,
        ),
    ]
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (_, index) in indexes() {
            manager.create_index(index).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (name, _) in indexes() {
            manager
                .drop_index(Index::drop().name(name).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
