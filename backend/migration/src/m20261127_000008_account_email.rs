//! An account is named by its email alone.
//!
//! - `users.username` goes: the sign-in box takes the email, every page
//!   names an account by its full name or its email. Its UNIQUE, its checks
//!   (`chk_users_username_lower`, `chk_users_username_no_at`,
//!   `chk_users_username_not_blank`) and its folded search index
//!   (`ix_users_username_fold`) go with the column.
//! - `email_verification_tokens.new_email`: the address a link confirms when
//!   it is not the account's own yet (an email change). NULL: the link
//!   confirms the account's current address. Opening a link with one moves
//!   the account to it, so the old address keeps working until the new one
//!   is proved.

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_check, drop_constraint, is_lower, not_blank, run, varchar_null, EmailVerificationTokens,
    Users,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// As `users.email`.
const EMAIL_LEN: u32 = 255;
const NEW_EMAIL_LOWER: &str = "chk_email_verification_tokens_new_email_lower";
const NEW_EMAIL_NOT_BLANK: &str = "chk_email_verification_tokens_new_email_not_blank";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::Username)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(EmailVerificationTokens::Table)
                    .add_column(varchar_null(EmailVerificationTokens::NewEmail, EMAIL_LEN))
                    .to_owned(),
            )
            .await?;
        // Folded like `users.email`, which it becomes.
        run(
            manager,
            &[
                add_check(
                    &EmailVerificationTokens::Table,
                    NEW_EMAIL_LOWER,
                    is_lower(EmailVerificationTokens::NewEmail),
                ),
                add_check(
                    &EmailVerificationTokens::Table,
                    NEW_EMAIL_NOT_BLANK,
                    not_blank(EmailVerificationTokens::NewEmail),
                ),
            ],
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run(
            manager,
            &[
                drop_constraint(&EmailVerificationTokens::Table, NEW_EMAIL_NOT_BLANK),
                drop_constraint(&EmailVerificationTokens::Table, NEW_EMAIL_LOWER),
            ],
        )
        .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(EmailVerificationTokens::Table)
                    .drop_column(EmailVerificationTokens::NewEmail)
                    .to_owned(),
            )
            .await?;
        // The names are gone; the column comes back empty, to be filled
        // before the older code runs against it.
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(varchar_null(Users::Username, 100))
                    .to_owned(),
            )
            .await
    }
}
