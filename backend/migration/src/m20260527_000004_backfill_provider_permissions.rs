/// Backfill: assign the four default provider permissions to every existing
/// user whose role code is 'provider', if not already granted.
///
/// This migration is idempotent — `ON CONFLICT DO NOTHING` makes it safe to
/// re-run. It is pure DML (no DDL changes) so the `down()` is a no-op; the
/// permissions can only be removed individually through the admin UI.
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260527_000004_backfill_provider_permissions"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO user_permissions (user_id, permission_id)
                 SELECT u.id, p.id
                 FROM users u
                 CROSS JOIN permissions p
                 INNER JOIN roles r ON r.id = u.role_id
                 WHERE r.code = 'provider'
                   AND p.code IN (
                       'ProviderProfileUpdate',
                       'ProviderListingCreate',
                       'ProviderListingUpdate',
                       'ProviderListingDelete'
                   )
                 ON CONFLICT DO NOTHING",
            )
            .await?;

        Ok(())
    }

    /// No-op: removing individual permission rows requires deliberate action
    /// through the admin panel. A bulk rollback would strip permissions from
    /// providers who may have had them legitimately before this migration.
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
