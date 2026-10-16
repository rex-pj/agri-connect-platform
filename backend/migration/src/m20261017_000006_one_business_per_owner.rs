//! One live business per provider account. The dashboard shows "the business
//! the account owns", so two would make one of them unmanageable; the admin
//! form already refuses it, and this index closes the race between two such
//! forms. Fails to apply if an account already owns two live businesses:
//! `SELECT owner_id FROM businesses WHERE owner_id IS NOT NULL AND deleted_at
//! IS NULL GROUP BY owner_id HAVING count(*) > 1` lists them.

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX uq_businesses_live_owner ON businesses (owner_id)
                 WHERE owner_id IS NOT NULL AND deleted_at IS NULL;",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS uq_businesses_live_owner;")
            .await?;
        Ok(())
    }
}
