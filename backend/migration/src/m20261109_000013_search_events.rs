//! `search_events`: what farmers looked for, for the demand report. Only the
//! words (lowercased, capped), the category filter, the area and how many
//! results came back; never who searched (no address, no session).

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE search_events (
                    id          BIGSERIAL    PRIMARY KEY,
                    query       VARCHAR(100),
                    category_id INT          REFERENCES categories (id) ON DELETE SET NULL,
                    location_id INT          REFERENCES locations (id) ON DELETE SET NULL,
                    results     INT          NOT NULL,
                    created_on  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
                );
                CREATE INDEX ix_search_events_created_on ON search_events (created_on);",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS search_events;")
            .await?;
        Ok(())
    }
}
