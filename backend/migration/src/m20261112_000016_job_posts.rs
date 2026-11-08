//! Job posts: a farmer says what needs doing ("phun thuốc 3 công lúa"), in
//! which service category and where; providers nearby with a live service
//! in that category are told by email and see it on their jobs page, with
//! the farmer's phone to call. Never public.

use sea_orm_migration::prelude::*;

use crate::schema::{
    add_check, int, int_null, is_vn_phone, references, run, serial_id, stamped, varchar,
    Categories, GeographyPoint, IndexMethod, JobPosts, Locations,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(JobPosts::Table)
                    .col(serial_id(JobPosts::Id))
                    .col(int(JobPosts::CategoryId))
                    .col(int_null(JobPosts::LocationId))
                    .col(
                        ColumnDef::new(JobPosts::Location)
                            .custom(GeographyPoint)
                            .not_null(),
                    )
                    .col(varchar(JobPosts::Description, 500))
                    .col(varchar(JobPosts::RequesterName, 100))
                    .col(varchar(JobPosts::RequesterPhone, 20))
                    .col(varchar(JobPosts::Status, 20).default("open"))
                    .col(stamped(JobPosts::CreatedOn))
                    .foreign_key(&mut references(
                        JobPosts::Table,
                        JobPosts::CategoryId,
                        Categories::Table,
                        Categories::Id,
                        ForeignKeyAction::Cascade,
                    ))
                    .foreign_key(&mut references(
                        JobPosts::Table,
                        JobPosts::LocationId,
                        Locations::Table,
                        Locations::Id,
                        ForeignKeyAction::SetNull,
                    ))
                    .to_owned(),
            )
            .await?;
        // Statuses frozen here; the live list is domain::constants::JOB_STATUSES.
        run(
            manager,
            &[
                add_check(
                    &JobPosts::Table,
                    "chk_job_posts_status",
                    Expr::col(JobPosts::Status).is_in(["open", "closed", "spam"]),
                ),
                add_check(
                    &JobPosts::Table,
                    "chk_job_posts_phone",
                    is_vn_phone(JobPosts::RequesterPhone),
                ),
            ],
        )
        .await?;
        for index in [
            Index::create()
                .name("ix_job_posts_location")
                .table(JobPosts::Table)
                .index_type(IndexType::Custom(IndexMethod::Gist.into_iden()))
                .col(JobPosts::Location)
                .to_owned(),
            Index::create()
                .name("ix_job_posts_created")
                .table(JobPosts::Table)
                .col((JobPosts::CreatedOn, IndexOrder::Desc))
                .col((JobPosts::Id, IndexOrder::Desc))
                .to_owned(),
            Index::create()
                .name("ix_job_posts_phone")
                .table(JobPosts::Table)
                .col(JobPosts::RequesterPhone)
                .col(JobPosts::CreatedOn)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(JobPosts::Table).if_exists().to_owned())
            .await
    }
}
