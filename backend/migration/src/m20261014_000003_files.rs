//! Uploaded images. The objects live in storage (a bucket or a directory);
//! this table records what is where, and products reference it by foreign key
//! so a deleted file can never leave a product pointing at nothing.

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE files (
                     id           UUID        PRIMARY KEY,
                     storage_key  TEXT        NOT NULL UNIQUE,
                     thumb_key    TEXT        NOT NULL UNIQUE,
                     content_type TEXT        NOT NULL,
                     byte_size    BIGINT      NOT NULL CHECK (byte_size > 0),
                     width        INT         NOT NULL CHECK (width > 0),
                     height       INT         NOT NULL CHECK (height > 0),
                     sha256       TEXT        NOT NULL,
                     uploaded_by  INT         REFERENCES users (id) ON DELETE SET NULL,
                     created_on   TIMESTAMPTZ NOT NULL DEFAULT now()
                 );
                 ALTER TABLE products
                     ADD COLUMN image_file_id UUID REFERENCES files (id) ON DELETE SET NULL;
                 -- Deleting a file looks up the products that reference it;
                 -- without this that is a full scan of products.
                 CREATE INDEX ix_products_image_file_id ON products (image_file_id);",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE products DROP COLUMN IF EXISTS image_file_id;
                 DROP TABLE IF EXISTS files;",
            )
            .await?;
        Ok(())
    }
}
