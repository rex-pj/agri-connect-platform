//! Accent-insensitive account search in the admin panel: /admin/users finds
//! an account by username, name or email typed without diacritics.
//!
//! Expression indexes on `fold(column)` (see `search.rs`), built from the
//! same `fold` the query uses, as for the services.

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

use crate::search::{fold, USER_SEARCH_INDEXES};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (index, table, column) in USER_SEARCH_INDEXES {
            db.execute_unprepared(&format!(
                "CREATE INDEX {index} ON {table} USING gin ({} gin_trgm_ops)",
                fold(column)
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (index, table, _) in USER_SEARCH_INDEXES {
            manager
                .drop_index(
                    Index::drop()
                        .name(*index)
                        .table(Alias::new(*table))
                        .if_exists()
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
