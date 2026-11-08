//! Refresh tokens: long-lived, single-use, stored as SHA-256 hashes only, so
//! a leaked table cannot be replayed. Tokens rotate on every use; all tokens
//! descended from one login share a `family_id`, and reusing a spent one
//! revokes the whole family (someone else has a copy).

use sea_orm_migration::prelude::*;

use crate::schema::{int, references, stamped, timestamp_null, RefreshTokens, Users};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RefreshTokens::Table)
                    .col(ColumnDef::new(RefreshTokens::Id).uuid().primary_key())
                    .col(ColumnDef::new(RefreshTokens::FamilyId).uuid().not_null())
                    .col(int(RefreshTokens::UserId))
                    .col(
                        ColumnDef::new(RefreshTokens::TokenHash)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(stamped(RefreshTokens::CreatedOn))
                    .col(timestamp_null(RefreshTokens::UsedAt))
                    .col(timestamp_null(RefreshTokens::RevokedAt))
                    .foreign_key(&mut references(
                        RefreshTokens::Table,
                        RefreshTokens::UserId,
                        Users::Table,
                        Users::Id,
                        ForeignKeyAction::Cascade,
                    ))
                    .to_owned(),
            )
            .await?;
        for (name, column) in [
            ("ix_refresh_tokens_family", RefreshTokens::FamilyId),
            ("ix_refresh_tokens_user", RefreshTokens::UserId),
            // The hourly purge deletes by expiry.
            ("ix_refresh_tokens_expires", RefreshTokens::ExpiresAt),
        ] {
            manager
                .create_index(
                    Index::create()
                        .name(name)
                        .table(RefreshTokens::Table)
                        .col(column)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RefreshTokens::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
