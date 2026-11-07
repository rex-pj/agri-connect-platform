//! Shapes of the reference-data catalogue seeded at every startup, and of the
//! reports the seeders return. The data itself lives in `application`.

/// Roles and permissions carry no wording: pages name them by code from the
/// locale file (`role.<code>`, `permission.<key>`). The database `name`
/// column gets the code, a fallback for rows the catalogue no longer has.
pub struct RoleSeed {
    pub code: &'static str,
}

pub struct PermissionSeed {
    /// Dotted key checked by use cases, e.g. `provider.verify`.
    pub key: &'static str,
    /// Role codes that get this permission by default. Applied once: on a fresh
    /// database, or when the permission row is first created.
    pub default_roles: &'static [&'static str],
}

pub struct CategorySeed {
    pub name: &'static str,
    pub slug: &'static str,
    pub kind: &'static str,
    pub icon: &'static str,
    pub color: &'static str,
    pub description: &'static str,
    pub display_order: i16,
}

pub struct LocationSeed {
    pub label: &'static str,
    pub lat: f64,
    pub lng: f64,
    pub display_order: i32,
}

/// A crop whose price the price page follows ("Lúa OM 18", "Sầu riêng Ri6").
pub struct CropSeed {
    pub name: &'static str,
    pub kind: &'static str,
    pub unit: &'static str,
    pub display_order: i32,
}

pub struct Catalogue {
    pub roles: &'static [RoleSeed],
    pub permissions: &'static [PermissionSeed],
    /// Inserted only into an empty table, so admin edits are never undone.
    pub categories: &'static [CategorySeed],
    /// Inserted only into an empty table, so admin edits are never undone.
    pub locations: &'static [LocationSeed],
    /// Inserted only into an empty table, so admin edits are never undone.
    pub crops: &'static [CropSeed],
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SeedReport {
    pub roles_inserted: u64,
    pub permissions_inserted: u64,
    pub grants_inserted: u64,
    /// True when this run applied the full default grant matrix (fresh database).
    pub bootstrapped_grants: bool,
    pub categories_inserted: u64,
    pub locations_inserted: u64,
    pub crops_inserted: u64,
    /// Permission keys present in the database but no longer in the catalogue.
    pub unknown_permissions: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DemoReport {
    pub businesses: u64,
    pub services: u64,
    pub products: u64,
}

/// What removing the demo data deleted: the businesses (their services and
/// products went with them) and the photos those products had, which the
/// caller removes from storage.
pub struct DemoRemoval {
    pub businesses: u64,
    pub image_files: Vec<uuid::Uuid>,
}
