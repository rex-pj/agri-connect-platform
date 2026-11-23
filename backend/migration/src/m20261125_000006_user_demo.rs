//! `users.is_demo`: the accounts the demo data at volume creates for its
//! providers and partners (`agri-connect demo-data`), so removing the demo
//! data deletes them too. Every other account, and every account created
//! before, is real (`false`).

use sea_orm_migration::prelude::*;

use crate::schema::{flag, Users};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(flag(Users::IsDemo, false))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::IsDemo)
                    .to_owned(),
            )
            .await
    }
}
