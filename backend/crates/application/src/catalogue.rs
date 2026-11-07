//! Reference data the app needs to run, seeded idempotently at every startup.
//!
//! Permission keys are the single source for authorization: use cases check
//! these constants, and the seeder creates exactly these rows. A key renamed
//! here leaves the old row behind (startup warns about it) and creates a new
//! one with default grants only, so renaming a key re-grants it to its
//! default roles.

use domain::{
    constants::{ROLE_ADMIN, ROLE_PROVIDER},
    models::catalogue::{Catalogue, CategorySeed, LocationSeed, PermissionSeed, RoleSeed},
};

// ── Permission keys ───────────────────────────────────────────────────────────

pub const PERM_ADMIN_DASHBOARD_VIEW: &str = "admin.dashboard.view";
pub const PERM_USER_READ: &str = "user.read";
pub const PERM_USER_ROLE_UPDATE: &str = "user.role.update";
pub const PERM_USER_STATUS_UPDATE: &str = "user.status.update";
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
pub const PERM_SYSTEM_DEMO_MANAGE: &str = "system.demo.manage";
pub const PERM_OWN_PROFILE_UPDATE: &str = "own.profile.update";
pub const PERM_OWN_LISTING_CREATE: &str = "own.listing.create";
pub const PERM_OWN_LISTING_UPDATE: &str = "own.listing.update";
pub const PERM_OWN_LISTING_DELETE: &str = "own.listing.delete";
pub const PERM_OWN_SERVICE_CREATE: &str = "own.service.create";
pub const PERM_OWN_SERVICE_UPDATE: &str = "own.service.update";
pub const PERM_OWN_SERVICE_DELETE: &str = "own.service.delete";
pub const PERM_OWN_INQUIRY_MANAGE: &str = "own.inquiry.manage";
pub const PERM_OWN_REVIEW_REPLY: &str = "own.review.reply";

const ADMIN: &[&str] = &[ROLE_ADMIN];
const PROVIDER: &[&str] = &[ROLE_PROVIDER];

pub static ROLES: &[RoleSeed] = &[
    RoleSeed { code: ROLE_ADMIN },
    RoleSeed {
        code: ROLE_PROVIDER,
    },
];

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
        key: PERM_SYSTEM_DEMO_MANAGE,
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
];

pub static CATEGORIES: &[CategorySeed] = &[
    CategorySeed {
        name: "Múc bùn",
        slug: "muc-bun",
        kind: "service",
        icon: "fa-solid fa-water",
        color: "#1E40AF",
        description: "Nạo vét kênh mương, ao hồ, ruộng lúa",
        display_order: 1,
    },
    CategorySeed {
        name: "Phun thuốc",
        slug: "phun-thuoc",
        kind: "service",
        icon: "fa-solid fa-spray-can-sparkles",
        color: "#065F46",
        description: "Phun thuốc bảo vệ thực vật, drone phun",
        display_order: 2,
    },
    CategorySeed {
        name: "Sửa máy nổ",
        slug: "sua-may-no",
        kind: "service",
        icon: "fa-solid fa-wrench",
        color: "#92400E",
        description: "Bảo dưỡng, sửa chữa máy bơm, máy gặt",
        display_order: 3,
    },
    CategorySeed {
        name: "Nông sản",
        slug: "nong-san",
        kind: "product",
        icon: "fa-solid fa-wheat-awn",
        color: "#065F46",
        description: "Gạo, trái cây, nông sản các loại",
        display_order: 1,
    },
    CategorySeed {
        name: "Thuốc BVTV",
        slug: "thuoc-bvtv",
        kind: "product",
        icon: "fa-solid fa-bug-slash",
        color: "#14532D",
        description: "Thuốc trừ sâu, diệt cỏ, bảo vệ thực vật",
        display_order: 2,
    },
    CategorySeed {
        name: "Nông cụ",
        slug: "nong-cu",
        kind: "product",
        icon: "fa-solid fa-trowel",
        color: "#92400E",
        description: "Dụng cụ canh tác, bình xịt, liềm, cuốc",
        display_order: 3,
    },
    CategorySeed {
        name: "Máy móc",
        slug: "may-moc",
        kind: "product",
        icon: "fa-solid fa-tractor",
        color: "#1E3A8A",
        description: "Máy bơm, máy gặt, drone phun thuốc",
        display_order: 4,
    },
    CategorySeed {
        name: "Phân bón",
        slug: "phan-bon",
        kind: "product",
        icon: "fa-solid fa-seedling",
        color: "#7F1D1D",
        description: "Phân NPK, phân hữu cơ, vôi bột",
        display_order: 5,
    },
];

// Location presets after the 2025 reorganisation (Nghị quyết 202/2025/QH15,
// in force 1 July 2025): 34 provinces, no district level. In the Mekong Delta:
//   Đồng Tháp ← Tiền Giang + Đồng Tháp     Vĩnh Long ← Vĩnh Long + Bến Tre + Trà Vinh
//   Tây Ninh  ← Long An + Tây Ninh         Cần Thơ   ← Cần Thơ + Hậu Giang + Sóc Trăng
//   An Giang  ← An Giang + Kiên Giang      Cà Mau    ← Cà Mau + Bạc Liêu
//   TP. Hồ Chí Minh ← TP.HCM + Bình Dương + Bà Rịa–Vũng Tàu
// A label is "province — ward or commune" with the unit's current name (it
// holds the centre of the former town of that name); the point is that
// town's centre. Only units whose current name and position were both
// checked are listed; admins add the others. Cai Lậy first: the launch area.
pub static LOCATIONS: &[LocationSeed] = &[
    LocationSeed {
        label: "Đồng Tháp — Cai Lậy",
        lat: 10.4170,
        lng: 106.0830,
        display_order: 1,
    },
    LocationSeed {
        label: "Đồng Tháp — Cái Bè",
        lat: 10.3330,
        lng: 106.0330,
        display_order: 2,
    },
    LocationSeed {
        label: "Đồng Tháp — Mỹ Tho",
        lat: 10.3581,
        lng: 106.3642,
        display_order: 3,
    },
    LocationSeed {
        label: "Đồng Tháp — Gò Công",
        lat: 10.3670,
        lng: 106.6670,
        display_order: 4,
    },
    LocationSeed {
        label: "Đồng Tháp — Cao Lãnh",
        lat: 10.4672,
        lng: 105.6303,
        display_order: 5,
    },
    LocationSeed {
        label: "Đồng Tháp — Sa Đéc",
        lat: 10.3000,
        lng: 105.7667,
        display_order: 6,
    },
    LocationSeed {
        label: "Đồng Tháp — Hồng Ngự",
        lat: 10.8330,
        lng: 105.2830,
        display_order: 7,
    },
    LocationSeed {
        label: "An Giang — Long Xuyên",
        lat: 10.3736,
        lng: 105.4458,
        display_order: 8,
    },
    LocationSeed {
        label: "An Giang — Châu Đốc",
        lat: 10.7000,
        lng: 105.1330,
        display_order: 9,
    },
    LocationSeed {
        label: "An Giang — Rạch Giá",
        lat: 9.9847,
        lng: 105.0992,
        display_order: 10,
    },
    LocationSeed {
        label: "An Giang — Hà Tiên",
        lat: 10.3858,
        lng: 104.4898,
        display_order: 11,
    },
    LocationSeed {
        label: "Cần Thơ — Ninh Kiều",
        lat: 10.0330,
        lng: 105.7830,
        display_order: 12,
    },
    LocationSeed {
        label: "Cần Thơ — Vị Thanh",
        lat: 9.7830,
        lng: 105.4670,
        display_order: 13,
    },
    LocationSeed {
        label: "Cần Thơ — Sóc Trăng",
        lat: 9.6028,
        lng: 105.9736,
        display_order: 14,
    },
    LocationSeed {
        label: "Vĩnh Long — Bến Tre",
        lat: 10.2308,
        lng: 106.3253,
        display_order: 15,
    },
    LocationSeed {
        label: "Vĩnh Long — Trà Vinh",
        lat: 9.9330,
        lng: 106.3500,
        display_order: 16,
    },
    LocationSeed {
        label: "Tây Ninh — Tân An",
        lat: 10.5333,
        lng: 106.4167,
        display_order: 17,
    },
    LocationSeed {
        label: "Cà Mau — An Xuyên",
        lat: 9.2239,
        lng: 105.1756,
        display_order: 18,
    },
    LocationSeed {
        label: "Cà Mau — Bạc Liêu",
        lat: 9.2830,
        lng: 105.7170,
        display_order: 19,
    },
    LocationSeed {
        label: "TP. Hồ Chí Minh — Sài Gòn",
        lat: 10.7756,
        lng: 106.7019,
        display_order: 20,
    },
];

pub static CATALOGUE: Catalogue = Catalogue {
    roles: ROLES,
    permissions: PERMISSIONS,
    categories: CATEGORIES,
    locations: LOCATIONS,
};

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use domain::constants::CATEGORY_TYPES;

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

    #[test]
    fn categories_use_known_types_and_unique_slugs() {
        let mut slugs = HashSet::new();
        for c in CATEGORIES {
            assert!(
                CATEGORY_TYPES.contains(&c.kind),
                "{} has unknown type {}",
                c.slug,
                c.kind
            );
            assert!(slugs.insert(c.slug), "duplicate slug {}", c.slug);
        }
    }

    #[test]
    fn location_labels_are_unique() {
        let mut labels = HashSet::new();
        for l in LOCATIONS {
            assert!(labels.insert(l.label), "duplicate location {}", l.label);
        }
    }
}
