//! Accent-insensitive search: `f_unaccent` plus trigram indexes on the folded
//! text, so `LIKE '%phan bon%'` matches "Phân bón" and uses an index.

use sea_orm_migration::prelude::*;

use crate::{
    schema::{create_extension, run, Ext, F_UNACCENT},
    search::{search_indexes, FoldIndex},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // In production the extensions already exist (deploy/db-init runs as the
        // superuser); IF NOT EXISTS then needs no privilege from the app role.
        let mut statements = vec![
            create_extension(Ext::Unaccent),
            create_extension(Ext::PgTrgm),
            F_UNACCENT.create_or_replace(),
        ];
        statements.extend(search_indexes().iter().map(FoldIndex::create));
        run(manager, &statements).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for index in search_indexes() {
            manager.drop_index(index.drop()).await?;
        }
        run(manager, &[F_UNACCENT.drop_if_exists(false)]).await
    }
}
