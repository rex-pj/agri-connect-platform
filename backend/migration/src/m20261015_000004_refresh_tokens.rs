//! Refresh tokens: long-lived, single-use, stored as SHA-256 hashes only, so
//! a leaked table cannot be replayed. Tokens rotate on every use; all tokens
//! descended from one login share a `family_id`, and reusing a spent one
//! revokes the whole family (someone else has a copy).

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE refresh_tokens (
                     id          UUID        PRIMARY KEY,
                     family_id   UUID        NOT NULL,
                     user_id     INT         NOT NULL REFERENCES users (id) ON DELETE CASCADE,
                     token_hash  TEXT        NOT NULL UNIQUE,
                     expires_at  TIMESTAMPTZ NOT NULL,
                     created_on  TIMESTAMPTZ NOT NULL DEFAULT now(),
                     used_at     TIMESTAMPTZ,
                     revoked_at  TIMESTAMPTZ
                 );
                 CREATE INDEX ix_refresh_tokens_family ON refresh_tokens (family_id);
                 CREATE INDEX ix_refresh_tokens_user ON refresh_tokens (user_id);
                 -- The hourly purge deletes by expiry.
                 CREATE INDEX ix_refresh_tokens_expires ON refresh_tokens (expires_at);",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS refresh_tokens;")
            .await?;
        Ok(())
    }
}
