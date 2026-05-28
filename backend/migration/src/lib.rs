pub use sea_orm_migration::prelude::*;

pub mod enums;
mod m20260524_000001_init;
mod m20260524_000002_seed;
mod m20260527_000003_audit_logs;
mod m20260527_000004_backfill_provider_permissions;

/// Primary migrator — runs automatically on startup.
/// Includes both schema and reference-data (seed) migrations so SeaORM can
/// reconcile any migration already applied by SeedMigrator without panicking.
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260524_000001_init::Migration),
            Box::new(m20260524_000002_seed::Migration),
            Box::new(m20260527_000003_audit_logs::Migration),
            Box::new(m20260527_000004_backfill_provider_permissions::Migration),
        ]
    }
}

/// Seed-data migrator — run only when the user explicitly triggers seeding
/// via the setup wizard. Never called from startup.
pub struct SeedMigrator;

#[async_trait::async_trait]
impl MigratorTrait for SeedMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260524_000001_init::Migration),
            Box::new(m20260524_000002_seed::Migration),
            Box::new(m20260527_000003_audit_logs::Migration),
            Box::new(m20260527_000004_backfill_provider_permissions::Migration),
        ]
    }
}
