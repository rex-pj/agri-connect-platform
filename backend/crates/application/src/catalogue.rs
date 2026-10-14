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
pub const PERM_PROVIDER_ACCOUNT_ASSIGN: &str = "provider.account.assign";
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

const ADMIN: &[&str] = &[ROLE_ADMIN];
const PROVIDER: &[&str] = &[ROLE_PROVIDER];

pub static ROLES: &[RoleSeed] = &[
    RoleSeed {
        code: ROLE_ADMIN,
        name: "Quản trị viên",
        description: "Quản lý toàn bộ nền tảng",
    },
    RoleSeed {
        code: ROLE_PROVIDER,
        name: "Nhà cung cấp",
        description: "Chủ doanh nghiệp quản lý hồ sơ và sản phẩm của mình",
    },
];

pub static PERMISSIONS: &[PermissionSeed] = &[
    PermissionSeed {
        key: PERM_ADMIN_DASHBOARD_VIEW,
        name: "Xem bảng điều khiển quản trị",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_USER_READ,
        name: "Xem người dùng",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_USER_ROLE_UPDATE,
        name: "Đổi vai trò người dùng",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_USER_STATUS_UPDATE,
        name: "Khoá / mở khoá người dùng",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_ROLE_READ,
        name: "Xem vai trò",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_ROLE_PERMISSION_MANAGE,
        name: "Phân quyền cho vai trò",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_CATEGORY_MANAGE,
        name: "Quản lý danh mục",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_LOCATION_MANAGE,
        name: "Quản lý khu vực",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_PROVIDER_READ,
        name: "Xem nhà cung cấp",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_PROVIDER_VERIFY,
        name: "Xác minh nhà cung cấp",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_PROVIDER_ACCOUNT_ASSIGN,
        name: "Gán tài khoản cho nhà cung cấp",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_LISTING_MODERATE,
        name: "Ẩn / xoá sản phẩm",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_SERVICE_MANAGE,
        name: "Quản lý dịch vụ",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_INQUIRY_READ,
        name: "Xem yêu cầu liên hệ",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_INQUIRY_MANAGE,
        name: "Cập nhật trạng thái yêu cầu",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_REVIEW_MODERATE,
        name: "Duyệt đánh giá",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_SYSTEM_DEMO_MANAGE,
        name: "Nạp / xoá dữ liệu mẫu",
        default_roles: ADMIN,
    },
    PermissionSeed {
        key: PERM_OWN_PROFILE_UPDATE,
        name: "Cập nhật hồ sơ doanh nghiệp của mình",
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_LISTING_CREATE,
        name: "Thêm sản phẩm của mình",
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_LISTING_UPDATE,
        name: "Sửa sản phẩm của mình",
        default_roles: PROVIDER,
    },
    PermissionSeed {
        key: PERM_OWN_LISTING_DELETE,
        name: "Xoá sản phẩm của mình",
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
        icon: "fa-solid fa-droplet",
        color: "#065F46",
        description: "Phun thuốc bảo vệ thực vật, drone phun",
        display_order: 2,
    },
    CategorySeed {
        name: "Sửa máy nổ",
        slug: "sua-may-no",
        kind: "service",
        icon: "fa-solid fa-gears",
        color: "#92400E",
        description: "Bảo dưỡng, sửa chữa máy bơm, máy gặt",
        display_order: 3,
    },
    CategorySeed {
        name: "Nông sản",
        slug: "nong-san",
        kind: "product",
        icon: "fa-solid fa-basket-shopping",
        color: "#065F46",
        description: "Gạo, trái cây, nông sản các loại",
        display_order: 1,
    },
    CategorySeed {
        name: "Thuốc BVTV",
        slug: "thuoc-bvtv",
        kind: "product",
        icon: "fa-solid fa-shield-halved",
        color: "#14532D",
        description: "Thuốc trừ sâu, diệt cỏ, bảo vệ thực vật",
        display_order: 2,
    },
    CategorySeed {
        name: "Nông cụ",
        slug: "nong-cu",
        kind: "product",
        icon: "fa-solid fa-screwdriver-wrench",
        color: "#92400E",
        description: "Dụng cụ canh tác, bình xịt, liềm, cuốc",
        display_order: 3,
    },
    CategorySeed {
        name: "Máy móc",
        slug: "may-moc",
        kind: "product",
        icon: "fa-solid fa-gears",
        color: "#1E3A8A",
        description: "Máy bơm, máy gặt, drone phun thuốc",
        display_order: 4,
    },
    CategorySeed {
        name: "Phân bón",
        slug: "phan-bon",
        kind: "product",
        icon: "fa-solid fa-bag-shopping",
        color: "#7F1D1D",
        description: "Phân NPK, phân hữu cơ, vôi bột",
        display_order: 5,
    },
];

// Mekong Delta + TP.HCM after the 2025 provincial merger (63 → 34 provinces):
// Long An ← Tiền Giang, Bến Tre ← Trà Vinh, Đồng Tháp ← Vĩnh Long,
// An Giang ← Kiên Giang, Cần Thơ ← Hậu Giang, Sóc Trăng ← Bạc Liêu.
pub static LOCATIONS: &[LocationSeed] = &[
    LocationSeed {
        label: "TP. Hồ Chí Minh",
        lat: 10.7769,
        lng: 106.7009,
        display_order: 1,
    },
    LocationSeed {
        label: "Cần Thơ",
        lat: 10.0452,
        lng: 105.7469,
        display_order: 2,
    },
    LocationSeed {
        label: "Cần Thơ — Vị Thanh",
        lat: 9.7856,
        lng: 105.4706,
        display_order: 3,
    },
    LocationSeed {
        label: "Long An — Tân An",
        lat: 10.5354,
        lng: 106.4120,
        display_order: 4,
    },
    LocationSeed {
        label: "Long An — Đức Hòa",
        lat: 10.8608,
        lng: 106.2873,
        display_order: 5,
    },
    LocationSeed {
        label: "Long An — Cai Lậy",
        lat: 10.4167,
        lng: 105.9500,
        display_order: 6,
    },
    LocationSeed {
        label: "Long An — Mỹ Tho",
        lat: 10.3600,
        lng: 106.3600,
        display_order: 7,
    },
    LocationSeed {
        label: "Long An — Cái Bè",
        lat: 10.3667,
        lng: 105.8833,
        display_order: 8,
    },
    LocationSeed {
        label: "Long An — Gò Công",
        lat: 10.3667,
        lng: 106.6667,
        display_order: 9,
    },
    LocationSeed {
        label: "Bến Tre",
        lat: 10.2433,
        lng: 106.3752,
        display_order: 10,
    },
    LocationSeed {
        label: "Bến Tre — Mỏ Cày",
        lat: 10.1364,
        lng: 106.3074,
        display_order: 11,
    },
    LocationSeed {
        label: "Bến Tre — Giồng Trôm",
        lat: 10.1775,
        lng: 106.4431,
        display_order: 12,
    },
    LocationSeed {
        label: "Bến Tre — Trà Vinh",
        lat: 9.9477,
        lng: 106.3422,
        display_order: 13,
    },
    LocationSeed {
        label: "Đồng Tháp — Cao Lãnh",
        lat: 10.4592,
        lng: 105.6330,
        display_order: 14,
    },
    LocationSeed {
        label: "Đồng Tháp — Sa Đéc",
        lat: 10.2938,
        lng: 105.7583,
        display_order: 15,
    },
    LocationSeed {
        label: "Đồng Tháp — Hồng Ngự",
        lat: 10.8167,
        lng: 105.3167,
        display_order: 16,
    },
    LocationSeed {
        label: "Đồng Tháp — Vĩnh Long",
        lat: 10.2500,
        lng: 105.9667,
        display_order: 17,
    },
    LocationSeed {
        label: "An Giang — Long Xuyên",
        lat: 10.3797,
        lng: 105.4358,
        display_order: 18,
    },
    LocationSeed {
        label: "An Giang — Châu Đốc",
        lat: 10.7044,
        lng: 105.1177,
        display_order: 19,
    },
    LocationSeed {
        label: "An Giang — Tân Châu",
        lat: 10.8026,
        lng: 105.2337,
        display_order: 20,
    },
    LocationSeed {
        label: "An Giang — Chợ Mới",
        lat: 10.4516,
        lng: 105.4477,
        display_order: 21,
    },
    LocationSeed {
        label: "An Giang — Rạch Giá",
        lat: 10.0125,
        lng: 105.0809,
        display_order: 22,
    },
    LocationSeed {
        label: "An Giang — Hà Tiên",
        lat: 10.3833,
        lng: 104.4833,
        display_order: 23,
    },
    LocationSeed {
        label: "Sóc Trăng",
        lat: 9.6031,
        lng: 105.9739,
        display_order: 24,
    },
    LocationSeed {
        label: "Sóc Trăng — Bạc Liêu",
        lat: 9.2827,
        lng: 105.7244,
        display_order: 25,
    },
    LocationSeed {
        label: "Cà Mau",
        lat: 9.1769,
        lng: 105.1524,
        display_order: 26,
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
