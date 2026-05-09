use sea_orm_migration::prelude::*;

use crate::enums::user::User;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .add_column(
                        ColumnDef::new(User::FailedLoginAttempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .add_column(
                        ColumnDef::new(User::LockedUntil)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .drop_column(User::FailedLoginAttempts)
                    .drop_column(User::LockedUntil)
                    .to_owned(),
            )
            .await
    }
}
