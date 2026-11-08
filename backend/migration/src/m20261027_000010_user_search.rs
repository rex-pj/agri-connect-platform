//! Accent-insensitive account search in the admin panel: /admin/users finds
//! an account by username, name or email typed without diacritics.
//!
//! Expression indexes on `fold(column)` (see `search.rs`), built from the
//! same `fold` the query uses, as for the services.

use sea_orm_migration::prelude::*;

use crate::{
    schema::run,
    search::{user_search_indexes, FoldIndex},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: Vec<String> = user_search_indexes()
            .iter()
            .map(FoldIndex::create)
            .collect();
        run(manager, &statements).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for index in user_search_indexes() {
            manager.drop_index(index.drop()).await?;
        }
        Ok(())
    }
}
