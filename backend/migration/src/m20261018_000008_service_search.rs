//! Accent-insensitive search over services: the provider directory and the
//! service listing match a service's name and description, so a farmer
//! typing "phun thuoc" finds a provider who only listed that service.
//!
//! Expression indexes on `fold(column)` (see `search.rs`): the schema
//! builder cannot express an index on a function call, so the statement is
//! built from the same `fold` the queries use.

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

use crate::search::{fold, SERVICE_SEARCH_INDEXES};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (index, table, column) in SERVICE_SEARCH_INDEXES {
            db.execute_unprepared(&format!(
                "CREATE INDEX {index} ON {table} USING gin ({} gin_trgm_ops)",
                fold(column)
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (index, table, _) in SERVICE_SEARCH_INDEXES {
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
