//! One optional photo per service (the provider's machine, a finished job),
//! referenced by foreign key like a product's, so a deleted file can never
//! leave a service pointing at nothing.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Services {
    Table,
    ImageFileId,
}

#[derive(DeriveIden)]
enum Files {
    Table,
    Id,
}

const INDEX: &str = "ix_services_image_file_id";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Services::Table)
                    .add_column(ColumnDef::new(Services::ImageFileId).uuid().null())
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_services_image_file_id")
                            .from_tbl(Services::Table)
                            .from_col(Services::ImageFileId)
                            .to_tbl(Files::Table)
                            .to_col(Files::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        // Deleting a file looks up the services that reference it; without
        // this that is a full scan of services.
        manager
            .create_index(
                Index::create()
                    .name(INDEX)
                    .table(Services::Table)
                    .col(Services::ImageFileId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(INDEX)
                    .table(Services::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Services::Table)
                    .drop_column(Services::ImageFileId)
                    .to_owned(),
            )
            .await
    }
}
