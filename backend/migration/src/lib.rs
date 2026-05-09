pub use sea_orm_migration::prelude::*;

pub mod enums;
mod m20260506_000001_init;
mod m20260506_000002_reference_data;
mod m20260506_000003_dev_seed;
mod m20260508_000004_user_lockout;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260506_000001_init::Migration),
            Box::new(m20260506_000002_reference_data::Migration),
            Box::new(m20260506_000003_dev_seed::Migration),
            Box::new(m20260508_000004_user_lockout::Migration),
        ]
    }
}
