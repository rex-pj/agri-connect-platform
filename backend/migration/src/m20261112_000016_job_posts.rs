//! Job posts: a farmer says what needs doing ("phun thuốc 3 công lúa"), in
//! which service category and where; providers nearby with a live service
//! in that category are told by email and see it on their jobs page, with
//! the farmer's phone to call. Never public.

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Keep the statuses in sync with domain::constants::JOB_STATUSES.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE job_posts (
                    id              SERIAL                 PRIMARY KEY,
                    category_id     INT                    NOT NULL REFERENCES categories (id) ON DELETE CASCADE,
                    location_id     INT                    REFERENCES locations (id) ON DELETE SET NULL,
                    location        GEOGRAPHY(POINT, 4326) NOT NULL,
                    description     VARCHAR(500)           NOT NULL,
                    requester_name  VARCHAR(100)           NOT NULL,
                    requester_phone VARCHAR(20)            NOT NULL,
                    status          VARCHAR(20)            NOT NULL DEFAULT 'open',
                    created_on      TIMESTAMPTZ            NOT NULL DEFAULT NOW(),
                    CONSTRAINT chk_job_posts_status CHECK (status IN ('open', 'closed', 'spam')),
                    CONSTRAINT chk_job_posts_phone CHECK (requester_phone ~ '^0[0-9]{9}$')
                );
                CREATE INDEX ix_job_posts_location ON job_posts USING GIST (location);
                CREATE INDEX ix_job_posts_created ON job_posts (created_on DESC, id DESC);
                CREATE INDEX ix_job_posts_phone ON job_posts (requester_phone, created_on);",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS job_posts;")
            .await?;
        Ok(())
    }
}
