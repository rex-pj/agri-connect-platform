//! Shapes of the platform catalogue seeded at every startup (roles and
//! permissions; the data itself lives in `application`), and of the reports
//! the seeders and the demo data return.

/// Roles and permissions carry no wording: pages name them by code from the
/// locale file (`role.<code>`, `permission.<key>`). The database `name`
/// column gets the code, a fallback for rows the catalogue no longer has.
pub struct RoleSeed {
    pub code: &'static str,
    /// May be granted any permission (the administrators). A role without
    /// it takes only the permissions meant for it (`default_roles`): the
    /// provider role is every self-registered account, and must never be
    /// handed an admin's key by a wrong tick.
    pub any_permission: bool,
}

pub struct PermissionSeed {
    /// Dotted key checked by use cases, e.g. `provider.verify`.
    pub key: &'static str,
    /// Role codes that get this permission by default. Applied once: on a fresh
    /// database, or when the permission row is first created.
    pub default_roles: &'static [&'static str],
}

/// The platform data that comes from code: roles and permissions, applied at
/// every startup. Categories, location presets and crops are not here: they
/// are demo data (`DemoDataRepositoryTrait`), and an admin adds the real ones.
pub struct Catalogue {
    pub roles: &'static [RoleSeed],
    pub permissions: &'static [PermissionSeed],
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SeedReport {
    pub roles_inserted: u64,
    pub permissions_inserted: u64,
    pub grants_inserted: u64,
    /// True when this run applied the full default grant matrix (fresh database).
    pub bootstrapped_grants: bool,
    /// Permission keys present in the database but no longer in the catalogue.
    pub unknown_permissions: Vec<String>,
}

/// What loading the demo data wrote, in `DemoCounts` terms. A demo
/// category, area or crop already present (same slug, label or name) is
/// reused, not counted; a row whose reference is gone (a renamed slug, an
/// area with no ward) is left out, and the counts show it.
pub type DemoReport = DemoCounts;

/// What removing the demo data did: the businesses it deleted (their
/// services, products, inquiries, reviews and call taps went with them), the
/// demo partners (contracts and sponsored shops followed), prices, job posts,
/// searches and alerts, the demo categories, areas and crops it deleted, those it
/// kept because real data uses them (they are real from then on), and the
/// photos of the deleted services and products, which the caller removes
/// from storage.
#[derive(Debug, Default)]
pub struct DemoRemoval {
    pub businesses: u64,
    pub activity_removed: u64,
    pub catalogue_removed: u64,
    pub catalogue_kept: u64,
    pub image_files: Vec<uuid::Uuid>,
}

/// The demo data present now, by kind. All of it goes when the demo data is
/// removed, but for a category, area or crop real data uses by then.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct DemoCounts {
    pub sectors: u64,
    pub alert_kinds: u64,
    pub commodities: u64,
    pub categories: u64,
    pub locations: u64,
    pub crops: u64,
    pub businesses: u64,
    pub services: u64,
    pub products: u64,
    pub inquiries: u64,
    pub reviews: u64,
    pub calls: u64,
    pub searches: u64,
    pub prices: u64,
    pub jobs: u64,
    pub partners: u64,
    pub contracts: u64,
    pub alerts: u64,
    /// Articles (news and guides).
    pub posts: u64,
    /// Information pages to start from.
    pub info_pages: u64,
    /// Provider and partner accounts of the demo data at volume.
    pub accounts: u64,
}

impl DemoCounts {
    /// True when any demo row is present.
    pub fn any(&self) -> bool {
        [
            self.sectors,
            self.alert_kinds,
            self.commodities,
            self.categories,
            self.locations,
            self.crops,
            self.businesses,
            self.searches,
            self.prices,
            self.jobs,
            self.partners,
            self.alerts,
            self.posts,
            self.info_pages,
            self.accounts,
        ]
        .iter()
        .any(|n| *n > 0)
    }
}

/// How much demo data `agri-connect demo-data` generates on top of the
/// file's: `providers` providers, everything around them in proportion
/// (services, supplies, accounts, contact requests, reviews, taps, prices,
/// job posts, alerts, partners). The same `seed` gives the same rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoVolume {
    pub providers: u32,
    pub seed: u64,
}

/// Platform data: what the platform runs on, never demo data and never
/// touched by removing it. Roles come from the catalogue at every startup;
/// provinces and wards from the bundled official dataset, the base an admin
/// picks the supported areas from.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PlatformCounts {
    pub roles: u64,
    pub provinces: u64,
    pub wards: u64,
}
