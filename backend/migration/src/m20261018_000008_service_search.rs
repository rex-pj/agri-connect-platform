//! Accent-insensitive search over services: the provider directory and the
//! service listing match a service's name and description, so a farmer
//! typing "phun thuoc" finds a provider who only listed that service.
//!
//! Expression indexes on `fold(column)` (see `search.rs`), built from the
//! same `fold` the queries use.

use sea_orm_migration::prelude::*;

use crate::{
    schema::run,
    search::{service_search_indexes, FoldIndex},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: Vec<String> = service_search_indexes()
            .iter()
            .map(FoldIndex::create)
            .collect();
        run(manager, &statements).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for index in service_search_indexes() {
            manager.drop_index(index.drop()).await?;
        }
        Ok(())
    }
}
