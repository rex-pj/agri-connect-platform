//! Accent-insensitive search: `f_unaccent` plus trigram indexes on the folded
//! text, so `LIKE '%phan bon%'` matches "Phân bón" and uses an index.

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

use crate::search::{fold, SEARCH_INDEXES};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        // In production the extensions already exist (deploy/db-init runs as the
        // superuser); IF NOT EXISTS then needs no privilege from the app role.
        db.execute_unprepared(
            "CREATE EXTENSION IF NOT EXISTS unaccent;
             CREATE EXTENSION IF NOT EXISTS pg_trgm;
             -- unaccent() is only STABLE (its dictionary could change), so it
             -- cannot appear in an index. This wrapper pins the dictionary and
             -- is declared IMMUTABLE; changing unaccent.rules would require a
             -- REINDEX of the indexes below.
             CREATE OR REPLACE FUNCTION f_unaccent(text) RETURNS text
               LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT
               AS $$ SELECT public.unaccent('public.unaccent'::regdictionary, $1) $$;",
        )
        .await?;
        for (index, table, column) in SEARCH_INDEXES {
            db.execute_unprepared(&format!(
                "CREATE INDEX {index} ON {table} USING gin ({} gin_trgm_ops)",
                fold(column)
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (index, _, _) in SEARCH_INDEXES {
            db.execute_unprepared(&format!("DROP INDEX IF EXISTS {index}"))
                .await?;
        }
        db.execute_unprepared("DROP FUNCTION IF EXISTS f_unaccent(text)")
            .await?;
        Ok(())
    }
}
