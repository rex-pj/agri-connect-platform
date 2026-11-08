//! One live business per provider account. The dashboard shows "the business
//! the account owns", so two would make one of them unmanageable; the admin
//! form already refuses it, and this index closes the race between two such
//! forms. Fails to apply if an account already owns two live businesses
//! (group the live businesses by `owner_id`, look for a count above one).

use sea_orm_migration::prelude::*;

use crate::schema::Businesses;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INDEX: &str = "uq_businesses_live_owner";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name(INDEX)
                    .table(Businesses::Table)
                    .col(Businesses::OwnerId)
                    .unique()
                    .and_where(Expr::col(Businesses::OwnerId).is_not_null())
                    .and_where(Expr::col(Businesses::DeletedAt).is_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(INDEX)
                    .table(Businesses::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
