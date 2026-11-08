//! Uploaded images. The objects live in storage (a bucket or a directory);
//! this table records what is where, and products reference it by foreign key
//! so a deleted file can never leave a product pointing at nothing.

use sea_orm_migration::prelude::*;

use crate::schema::{int, int_null, references, stamped, Files, Products, Users};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `TEXT NOT NULL`.
fn text(c: Files) -> ColumnDef {
    ColumnDef::new(c).text().not_null().to_owned()
}

/// `column > 0`.
fn positive(c: Files) -> SimpleExpr {
    Expr::col(c).gt(0)
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Files::Table)
                    .col(ColumnDef::new(Files::Id).uuid().primary_key())
                    .col(text(Files::StorageKey).unique_key())
                    .col(text(Files::ThumbKey).unique_key())
                    .col(text(Files::ContentType))
                    .col(
                        ColumnDef::new(Files::ByteSize)
                            .big_integer()
                            .not_null()
                            .check(positive(Files::ByteSize)),
                    )
                    .col(int(Files::Width).check(positive(Files::Width)))
                    .col(int(Files::Height).check(positive(Files::Height)))
                    .col(text(Files::Sha256))
                    .col(int_null(Files::UploadedBy))
                    .col(stamped(Files::CreatedOn))
                    .foreign_key(&mut references(
                        Files::Table,
                        Files::UploadedBy,
                        Users::Table,
                        Users::Id,
                        ForeignKeyAction::SetNull,
                    ))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Products::Table)
                    .add_column(ColumnDef::new(Products::ImageFileId).uuid())
                    .add_foreign_key(
                        // The name Postgres gives an inline REFERENCES.
                        TableForeignKey::new()
                            .name("products_image_file_id_fkey")
                            .from_tbl(Products::Table)
                            .from_col(Products::ImageFileId)
                            .to_tbl(Files::Table)
                            .to_col(Files::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        // Deleting a file looks up the products that reference it; without
        // this that is a full scan of products.
        manager
            .create_index(
                Index::create()
                    .name("ix_products_image_file_id")
                    .table(Products::Table)
                    .col(Products::ImageFileId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Products::Table)
                    .drop_column(Products::ImageFileId)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Files::Table).if_exists().to_owned())
            .await
    }
}
