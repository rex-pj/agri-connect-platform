//! The information pages: the terms, the privacy and personal data
//! policies, the rules for providers and listings, the complaints and
//! takedown procedures, the legal entity, the contact page.
//!
//! - `info_pages`: one row per page. `code` is its address (`/info/{code}`)
//!   and never changes; `stage` who sees it (`on`, `preview`: only an
//!   account with `feature.preview`, `off`: nobody), as a feature's stage;
//!   `in_footer` and `footer_group` where the footer lists it.
//! - `info_page_revisions`: its text, one row per version, never edited:
//!   the version in force is the latest whose `effective_on` has come, so a
//!   new policy can be written ahead of the day it applies and the former
//!   ones stay readable.

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_check, by_user, create_updated_on_trigger, flag, int, matches, not_blank, references, run,
    serial_id, stamped, varchar, varchar_null, InfoPageRevisions, InfoPages, SLUG_PATTERN,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Frozen here; the live figures are `application::constants`.
const CODE_LEN: u32 = 60;
const TITLE_LEN: u32 = 160;
const ICON_LEN: u32 = 64;
const NOTE_LEN: u32 = 300;
/// Frozen here; the live lists are `application::info_pages`.
const STAGES: [&str; 3] = ["on", "preview", "off"];
const FOOTER_GROUPS: [&str; 2] = ["about", "policy"];
const STAGE_LEN: u32 = 10;
const GROUP_LEN: u32 = 10;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(InfoPages::Table)
                    .col(serial_id(InfoPages::Id))
                    .col(varchar(InfoPages::Code, CODE_LEN).unique_key())
                    .col(varchar(InfoPages::Title, TITLE_LEN))
                    .col(varchar_null(InfoPages::Icon, ICON_LEN))
                    .col(varchar(InfoPages::Stage, STAGE_LEN).default("off"))
                    .col(flag(InfoPages::InFooter, false))
                    .col(varchar(InfoPages::FooterGroup, GROUP_LEN).default("policy"))
                    .col(int(InfoPages::DisplayOrder).default(0))
                    .col(flag(InfoPages::IsDemo, false))
                    .col(stamped(InfoPages::CreatedOn))
                    .col(stamped(InfoPages::UpdatedOn))
                    .col(ColumnDef::new(InfoPages::CreatedBy).integer())
                    .col(ColumnDef::new(InfoPages::UpdatedBy).integer())
                    .foreign_key(&mut by_user(InfoPages::Table, InfoPages::CreatedBy))
                    .foreign_key(&mut by_user(InfoPages::Table, InfoPages::UpdatedBy))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(InfoPageRevisions::Table)
                    .col(serial_id(InfoPageRevisions::Id))
                    .col(int(InfoPageRevisions::PageId))
                    .col(ColumnDef::new(InfoPageRevisions::Body).text().not_null())
                    .col(
                        ColumnDef::new(InfoPageRevisions::EffectiveOn)
                            .date()
                            .not_null(),
                    )
                    .col(varchar_null(InfoPageRevisions::ChangeNote, NOTE_LEN))
                    .col(stamped(InfoPageRevisions::CreatedOn))
                    .col(ColumnDef::new(InfoPageRevisions::CreatedBy).integer())
                    .foreign_key(&mut references(
                        InfoPageRevisions::Table,
                        InfoPageRevisions::PageId,
                        InfoPages::Table,
                        InfoPages::Id,
                        ForeignKeyAction::Cascade,
                    ))
                    .foreign_key(&mut by_user(
                        InfoPageRevisions::Table,
                        InfoPageRevisions::CreatedBy,
                    ))
                    .to_owned(),
            )
            .await?;
        // The version in force: the latest come of one page.
        manager
            .create_index(
                Index::create()
                    .name("ix_info_page_revisions_page_effective")
                    .table(InfoPageRevisions::Table)
                    .col(InfoPageRevisions::PageId)
                    .col((InfoPageRevisions::EffectiveOn, IndexOrder::Desc))
                    .col((InfoPageRevisions::Id, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;
        run(
            manager,
            &[
                add_check(
                    &InfoPages::Table,
                    "chk_info_pages_code",
                    matches(InfoPages::Code, SLUG_PATTERN),
                ),
                add_check(
                    &InfoPages::Table,
                    "chk_info_pages_title",
                    not_blank(InfoPages::Title),
                ),
                add_check(
                    &InfoPages::Table,
                    "chk_info_pages_stage",
                    Expr::col(InfoPages::Stage).is_in(STAGES),
                ),
                add_check(
                    &InfoPages::Table,
                    "chk_info_pages_footer_group",
                    Expr::col(InfoPages::FooterGroup).is_in(FOOTER_GROUPS),
                ),
                add_check(
                    &InfoPageRevisions::Table,
                    "chk_info_page_revisions_body",
                    not_blank(InfoPageRevisions::Body),
                ),
                create_updated_on_trigger(&InfoPages::Table),
            ],
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(InfoPageRevisions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(InfoPages::Table).to_owned())
            .await
    }
}
