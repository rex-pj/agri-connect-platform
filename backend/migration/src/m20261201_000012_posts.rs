//! Articles: the news (what happens now: a disease, a price, a policy of
//! the province) and the guides (what stays true: how to treat a disease,
//! the season's calendar), one table told apart by `kind`, since both have
//! the same boxes and are listed the same way.
//!
//! A post is a draft, published or hidden; `published_on` is set the first
//! time it is published and orders the lists. It may concern one sector,
//! one province and one commodity: each points to its table and is cleared
//! (`SET NULL`) when that row goes, so removing the demo data never fails on
//! a real post. Its cover is an uploaded image (`files`), cleared likewise.

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_check, by_user, create_updated_on_trigger, flag, not_blank, references, run, serial_id,
    stamped, timestamp_null, varchar, varchar_null, CodedLists, Commodities, Files, Posts,
    Provinces, Sectors,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Frozen here; the live figures are `application::constants`.
const TITLE_LEN: u32 = 160;
const SUMMARY_LEN: u32 = 300;
/// Frozen here; the live lists are `application::posts`.
const KINDS: [&str; 2] = ["news", "guide"];
const STATUSES: [&str; 3] = ["draft", "published", "hidden"];
const CODE_LEN: u32 = 10;
/// The columns that point to a sector's code and a province's code.
const SECTOR_LEN: u32 = 50;
const PROVINCE_LEN: u32 = 2;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Posts::Table)
                    .col(serial_id(Posts::Id))
                    .col(varchar(Posts::Kind, CODE_LEN))
                    .col(varchar(Posts::Title, TITLE_LEN))
                    .col(varchar(Posts::Summary, SUMMARY_LEN).default(""))
                    .col(ColumnDef::new(Posts::Body).text().not_null())
                    .col(ColumnDef::new(Posts::CoverFileId).uuid())
                    .col(varchar_null(Posts::Sector, SECTOR_LEN))
                    .col(varchar_null(Posts::ProvinceCode, PROVINCE_LEN))
                    .col(ColumnDef::new(Posts::CommodityId).integer())
                    .col(varchar(Posts::Status, CODE_LEN).default("draft"))
                    .col(timestamp_null(Posts::PublishedOn))
                    .col(flag(Posts::IsPinned, false))
                    .col(flag(Posts::IsDemo, false))
                    .col(stamped(Posts::CreatedOn))
                    .col(stamped(Posts::UpdatedOn))
                    .col(ColumnDef::new(Posts::CreatedBy).integer())
                    .col(ColumnDef::new(Posts::UpdatedBy).integer())
                    .foreign_key(&mut references(
                        Posts::Table,
                        Posts::CoverFileId,
                        Files::Table,
                        Files::Id,
                        ForeignKeyAction::SetNull,
                    ))
                    .foreign_key(&mut references(
                        Posts::Table,
                        Posts::Sector,
                        Sectors::Table,
                        CodedLists::Code,
                        ForeignKeyAction::SetNull,
                    ))
                    .foreign_key(&mut references(
                        Posts::Table,
                        Posts::ProvinceCode,
                        Provinces::Table,
                        Provinces::Code,
                        ForeignKeyAction::SetNull,
                    ))
                    .foreign_key(&mut references(
                        Posts::Table,
                        Posts::CommodityId,
                        Commodities::Table,
                        Commodities::Id,
                        ForeignKeyAction::SetNull,
                    ))
                    .foreign_key(&mut by_user(Posts::Table, Posts::CreatedBy))
                    .foreign_key(&mut by_user(Posts::Table, Posts::UpdatedBy))
                    .to_owned(),
            )
            .await?;
        // The public lists: one kind's published posts, pinned first, the
        // newest first.
        manager
            .create_index(
                Index::create()
                    .name("ix_posts_public")
                    .table(Posts::Table)
                    .col(Posts::Status)
                    .col(Posts::Kind)
                    .col((Posts::IsPinned, IndexOrder::Desc))
                    .col((Posts::PublishedOn, IndexOrder::Desc))
                    .col((Posts::Id, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;
        run(
            manager,
            &[
                add_check(
                    &Posts::Table,
                    "chk_posts_kind",
                    Expr::col(Posts::Kind).is_in(KINDS),
                ),
                add_check(
                    &Posts::Table,
                    "chk_posts_status",
                    Expr::col(Posts::Status).is_in(STATUSES),
                ),
                add_check(&Posts::Table, "chk_posts_title", not_blank(Posts::Title)),
                add_check(&Posts::Table, "chk_posts_body", not_blank(Posts::Body)),
                // A post once published has its day, also when hidden later.
                add_check(
                    &Posts::Table,
                    "chk_posts_published_on",
                    Expr::col(Posts::Status)
                        .ne("published")
                        .or(Expr::col(Posts::PublishedOn).is_not_null()),
                ),
                create_updated_on_trigger(&Posts::Table),
            ],
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Posts::Table).to_owned())
            .await
    }
}
