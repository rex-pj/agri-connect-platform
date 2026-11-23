//! A username never holds `@`.
//!
//! The sign-in box takes a username or an email: with an `@` it is an
//! email, without one a username. A username with an `@` could be someone
//! else's email, and the box could not tell which account was meant.

use sea_orm_migration::prelude::*;

use crate::schema::{add_check, drop_constraint, run, Users};

#[derive(DeriveMigrationName)]
pub struct Migration;

const CHECK: &str = "chk_users_username_no_at";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run(
            manager,
            &[add_check(
                &Users::Table,
                CHECK,
                Expr::col(Users::Username).not_like("%@%"),
            )],
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run(manager, &[drop_constraint(&Users::Table, CHECK)]).await
    }
}
