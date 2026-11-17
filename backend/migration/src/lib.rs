//! Schema migrations. DDL only; platform data (roles, permissions, provinces
//! and wards) is seeded at startup, demo data (the catalogue, areas, crops,
//! listings) only on request.
//!
//! Append-only from the baseline on: an applied migration is never removed,
//! and its schema never changes. sea-orm refuses to start against a database
//! whose recorded migrations have no file, and editing one leaves existing
//! databases on the old definition.

pub use sea_orm_migration::prelude::*;

mod m20261120_000001_baseline;
mod m20261121_000002_email_verification;
mod m20261122_000003_scale;
pub mod schema;
pub mod search;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20261120_000001_baseline::Migration),
            Box::new(m20261121_000002_email_verification::Migration),
            Box::new(m20261122_000003_scale::Migration),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_names_strictly_increase() {
        let names: Vec<String> = Migrator::migrations()
            .iter()
            .map(|m| m.name().to_owned())
            .collect();
        for pair in names.windows(2) {
            if let [a, b] = pair {
                assert!(
                    a < b,
                    "migration {b} must sort after {a}; append new migrations at the end"
                );
            }
        }
    }

    #[test]
    fn every_migration_file_is_registered() {
        // A migration file that is not in the list never runs, silently.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let registered: Vec<String> = Migrator::migrations()
            .iter()
            .map(|m| m.name().to_owned())
            .collect();
        let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string());
        assert!(entries.is_ok(), "cannot read {}", dir.display());
        for entry in entries.into_iter().flatten().flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if let Some(stem) = file.strip_suffix(".rs").filter(|s| {
                s.starts_with('m') && s.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
            }) {
                assert!(
                    registered.iter().any(|r| r == stem),
                    "{file} is not registered in Migrator"
                );
            }
        }
    }
}
