//! The platform data that comes from code: roles and permissions, seeded
//! idempotently at every startup. Categories, area presets and crops are demo
//! data (`infrastructure/src/demo/data.json`); an admin adds the real ones.
//!
//! Permission keys are the single source for authorization: use cases check
//! these constants, and the seeder creates exactly these rows. A key renamed
//! here leaves the old row behind (startup warns about it) and creates a new
//! one with default grants only, so renaming a key re-grants it to its
//! default roles.

use domain::{
    constants::{ROLE_ADMIN, ROLE_PARTNER, ROLE_PROVIDER},
    models::catalogue::{Catalogue, PermissionSeed, RoleSeed},
};

// ── Permission keys ───────────────────────────────────────────────────────────

pub const PERM_ADMIN_DASHBOARD_VIEW: &str = "admin.dashboard.view";
pub const PERM_USER_READ: &str = "user.read";
pub const PERM_USER_ROLE_UPDATE: &str = "user.role.update";
pub const PERM_USER_STATUS_UPDATE: &str = "user.status.update";
/// An account's name and email (through the link sent to the new one), and
/// the password-reset link sent for it: never the password itself.
pub const PERM_USER_UPDATE: &str = "user.update";
pub const PERM_ROLE_READ: &str = "role.read";
pub const PERM_ROLE_PERMISSION_MANAGE: &str = "role.permission.manage";
pub const PERM_CATEGORY_MANAGE: &str = "category.manage";
pub const PERM_LOCATION_MANAGE: &str = "location.manage";
pub const PERM_PROVIDER_READ: &str = "provider.read";
pub const PERM_PROVIDER_VERIFY: &str = "provider.verify";
pub const PERM_PROVIDER_STATUS_UPDATE: &str = "provider.status.update";
pub const PERM_PROVIDER_ACCOUNT_ASSIGN: &str = "provider.account.assign";
pub const PERM_PROVIDER_MANAGE: &str = "provider.manage";
pub const PERM_LISTING_MODERATE: &str = "listing.moderate";
pub const PERM_SERVICE_MANAGE: &str = "service.manage";
pub const PERM_INQUIRY_READ: &str = "inquiry.read";
pub const PERM_INQUIRY_MANAGE: &str = "inquiry.manage";
pub const PERM_REVIEW_MODERATE: &str = "review.moderate";
pub const PERM_REPORT_DEMAND_READ: &str = "report.demand.read";
pub const PERM_PARTNER_MANAGE: &str = "partner.manage";
pub const PERM_PARTNER_REPORT_READ: &str = "partner.report.read";
pub const PERM_PRICE_MANAGE: &str = "price.manage";
pub const PERM_JOB_MODERATE: &str = "job.moderate";
pub const PERM_ALERT_MANAGE: &str = "alert.manage";
pub const PERM_CONTENT_PAGE_MANAGE: &str = "content.page.manage";
pub const PERM_SYSTEM_DEMO_MANAGE: &str = "system.demo.manage";
pub const PERM_SETTING_MANAGE: &str = "setting.manage";
pub const PERM_FEATURE_PREVIEW: &str = "feature.preview";
pub const PERM_OWN_PROFILE_UPDATE: &str = "own.profile.update";
pub const PERM_OWN_LISTING_CREATE: &str = "own.listing.create";
pub const PERM_OWN_LISTING_UPDATE: &str = "own.listing.update";
pub const PERM_OWN_LISTING_DELETE: &str = "own.listing.delete";
pub const PERM_OWN_SERVICE_CREATE: &str = "own.service.create";
pub const PERM_OWN_SERVICE_UPDATE: &str = "own.service.update";
pub const PERM_OWN_SERVICE_DELETE: &str = "own.service.delete";
pub const PERM_OWN_INQUIRY_MANAGE: &str = "own.inquiry.manage";
pub const PERM_OWN_REVIEW_REPLY: &str = "own.review.reply";
pub const PERM_OWN_JOB_READ: &str = "own.job.read";

const ADMIN: &[&str] = &[ROLE_ADMIN];
const PROVIDER: &[&str] = &[ROLE_PROVIDER];
const PARTNER: &[&str] = &[ROLE_PARTNER];

pub static ROLES: &[RoleSeed] = &[
    RoleSeed {
        code: ROLE_ADMIN,
        any_permission: true,
    },
    RoleSeed {
        code: ROLE_PARTNER,
        any_permission: false,
    },
    RoleSeed {
        code: ROLE_PROVIDER,
        any_permission: false,
    },
];

/// Whether the role `role_code` may be granted `key`: any permission for a
/// role with `any_permission`, else only one meant for that role. A role or
/// a key the catalogue does not know is granted nothing.
pub fn grantable(role_code: &str, key: &str) -> bool {
    let Some(role) = ROLES.iter().find(|r| r.code == role_code) else {
        return false;
    };
    PERMISSIONS
        .iter()
        .find(|p| p.key == key)
        .is_some_and(|p| role.any_permission || p.default_roles.contains(&role.code))
}

pub static PERMISSIONS: &[PermissionSeed] = &[
    PermissionSeed {
        key: PERM_ADMIN_DASHBOARD_VIEW,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_USER_READ,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_USER_ROLE_UPDATE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_USER_STATUS_UPDATE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_USER_UPDATE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_ROLE_READ,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_ROLE_PERMISSION_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_CATEGORY_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_LOCATION_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_PROVIDER_READ,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_PROVIDER_VERIFY,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_PROVIDER_STATUS_UPDATE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_PROVIDER_ACCOUNT_ASSIGN,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_PROVIDER_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_LISTING_MODERATE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_SERVICE_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_INQUIRY_READ,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_INQUIRY_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_REVIEW_MODERATE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_REPORT_DEMAND_READ,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_PARTNER_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_PARTNER_REPORT_READ,
        default_roles: PARTNER,
    },
    PermissionSeed {
        key: PERM_PRICE_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_JOB_MODERATE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_ALERT_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_CONTENT_PAGE_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_SYSTEM_DEMO_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_SETTING_MANAGE,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_FEATURE_PREVIEW,
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_OWN_PROFILE_UPDATE,
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_LISTING_CREATE,
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_LISTING_UPDATE,
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_LISTING_DELETE,
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_SERVICE_CREATE,
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_SERVICE_UPDATE,
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_SERVICE_DELETE,
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_INQUIRY_MANAGE,
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_REVIEW_REPLY,
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_JOB_READ,
        default_roles: PROVIDER,
    },
];

pub static CATALOGUE: Catalogue = Catalogue {
    roles: ROLES,
    permissions: PERMISSIONS,
};

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn permission_keys_are_unique_and_dotted() {
        let mut seen = HashSet::new();
        for p in PERMISSIONS {
            assert!(seen.insert(p.key), "duplicate permission key {}", p.key);
            assert!(
                p.key.split('.').count() >= 2
                    && p.key.chars().all(|c| c.is_ascii_lowercase() || c == '.'),
                "permission key {} is not a dotted lowercase key",
                p.key
            );
        }
    }

    #[test]
    fn only_administrators_can_be_granted_any_permission() {
        assert!(grantable(ROLE_ADMIN, PERM_USER_READ));
        assert!(grantable(ROLE_ADMIN, PERM_OWN_SERVICE_CREATE));
        assert!(grantable(ROLE_PROVIDER, PERM_OWN_SERVICE_CREATE));
        assert!(!grantable(ROLE_PROVIDER, PERM_USER_READ));
        assert!(!grantable(ROLE_PROVIDER, PERM_PARTNER_REPORT_READ));
        assert!(grantable(ROLE_PARTNER, PERM_PARTNER_REPORT_READ));
        assert!(!grantable(ROLE_PARTNER, PERM_PARTNER_MANAGE));
        assert!(!grantable("khach", PERM_OWN_SERVICE_CREATE));
        assert!(!grantable(ROLE_ADMIN, "no.such.key"));
        // Every default grant is one the role may hold.
        for p in PERMISSIONS {
            for role in p.default_roles {
                assert!(grantable(role, p.key), "{role} cannot hold {}", p.key);
            }
        }
    }

    #[test]
    fn default_roles_exist_in_the_role_catalogue() {
        let roles: HashSet<_> = ROLES.iter().map(|r| r.code).collect();
        for p in PERMISSIONS {
            for role in p.default_roles {
                assert!(roles.contains(role), "{} grants unknown role {role}", p.key);
            }
        }
    }

    #[test]
    fn every_permission_has_a_default_role() {
        // A permission nobody holds by default can only be granted by an admin
        // who already knows it exists; the role grid would show it unchecked.
        for p in PERMISSIONS {
            assert!(!p.default_roles.is_empty(), "{} has no default role", p.key);
        }
    }
}
