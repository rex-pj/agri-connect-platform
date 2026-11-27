//! What an account agreed to when it signed up: one row per information
//! page it was asked to accept (the terms, the personal data policy), with
//! the very version it read, as the personal data law asks a platform to
//! prove. Never edited; gone with the account.

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_reference, add_unique, int, references, run, serial_id, stamped, Consents,
    InfoPageRevisions, Users,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // A consent names a version of the page it names: a reference over
        // both columns needs them unique together.
        run(
            manager,
            &[add_unique(
                &InfoPageRevisions::Table,
                "uq_info_page_revisions_page",
                &[&InfoPageRevisions::Id, &InfoPageRevisions::PageId],
            )],
        )
        .await?;
        manager
            .create_table(
                Table::create()
                    .table(Consents::Table)
                    .col(serial_id(Consents::Id))
                    .col(int(Consents::UserId))
                    .col(int(Consents::PageId))
                    .col(int(Consents::RevisionId))
                    .col(stamped(Consents::AcceptedOn))
                    .foreign_key(&mut references(
                        Consents::Table,
                        Consents::UserId,
                        Users::Table,
                        Users::Id,
                        ForeignKeyAction::Cascade,
                    ))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ix_consents_user")
                    .table(Consents::Table)
                    .col(Consents::UserId)
                    .to_owned(),
            )
            .await?;
        run(
            manager,
            &[add_reference(
                &Consents::Table,
                "fk_consents_revision",
                &[&Consents::RevisionId, &Consents::PageId],
                &InfoPageRevisions::Table,
                &[&InfoPageRevisions::Id, &InfoPageRevisions::PageId],
                &[],
            )],
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Consents::Table).to_owned())
            .await?;
        run(
            manager,
            &[crate::schema::drop_constraint(
                &InfoPageRevisions::Table,
                "uq_info_page_revisions_page",
            )],
        )
        .await
    }
}
