//! Email verification: an account's address counts as its own only once the
//! link sent to it was opened. Until then nothing is emailed to it but that
//! link and a password reset, so a sign-up with a stranger's address cannot
//! make the platform write to them.
//!
//! - `users.email_verified_at`: when the address was confirmed; NULL until
//!   then. Accounts an admin creates are stamped by the application.
//! - `email_verification_tokens`: the links sent, each stored as the SHA-256
//!   of its token, single-use, with an expiry.

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_check, int, matches, references, run, serial_id, stamped, timestamp_null, varchar,
    EmailVerificationTokens, Users, SHA256_HEX_PATTERN,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// A SHA-256 in hex.
const TOKEN_HASH_LEN: u32 = 64;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(timestamp_null(Users::EmailVerifiedAt))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(EmailVerificationTokens::Table)
                    .col(serial_id(EmailVerificationTokens::Id))
                    .col(int(EmailVerificationTokens::UserId))
                    .col(varchar(EmailVerificationTokens::TokenHash, TOKEN_HASH_LEN).unique_key())
                    .col(
                        ColumnDef::new(EmailVerificationTokens::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(timestamp_null(EmailVerificationTokens::UsedAt))
                    .col(stamped(EmailVerificationTokens::CreatedOn))
                    .foreign_key(&mut references(
                        EmailVerificationTokens::Table,
                        EmailVerificationTokens::UserId,
                        Users::Table,
                        Users::Id,
                        ForeignKeyAction::Cascade,
                    ))
                    .to_owned(),
            )
            .await?;
        run(
            manager,
            &[add_check(
                &EmailVerificationTokens::Table,
                "chk_email_verification_tokens_hash",
                matches(EmailVerificationTokens::TokenHash, SHA256_HEX_PATTERN),
            )],
        )
        .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_email_verification_tokens_user_id")
                    .table(EmailVerificationTokens::Table)
                    .col(EmailVerificationTokens::UserId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(EmailVerificationTokens::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::EmailVerifiedAt)
                    .to_owned(),
            )
            .await
    }
}
