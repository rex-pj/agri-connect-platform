// ── Pagination ─────────────────────────────────────────────────────────────────
pub const PAGE_SIZE:       i64 = 20;
pub const ADMIN_PAGE_SIZE: i64 = 25;
pub const MAX_PAGE:        i64 = 10_000;

// ── Featured counts ────────────────────────────────────────────────────────────
pub const FEATURED_PROVIDERS_COUNT: i64 = 6;
pub const FEATURED_PRODUCTS_COUNT:  i64 = 8;

// ── Authentication ─────────────────────────────────────────────────────────────
pub const JWT_EXPIRY_HOURS:            i64 = 1;
pub const PASSWORD_RESET_EXPIRY_HOURS: i64 = 1;
pub const LOGIN_FAIL_MAX_ATTEMPTS:     i32 = 5;
pub const LOGIN_LOCK_MINUTES:          i64 = 15;

// ── Validation ─────────────────────────────────────────────────────────────────
pub const USERNAME_MIN_LEN: usize = 3;
pub const PASSWORD_MIN_LEN: usize = 8;

// ── Geography ─────────────────────────────────────────────────────────────────
pub const LAT_MIN:          f64 = -90.0;
pub const LAT_MAX:          f64 =  90.0;
pub const LNG_MIN:          f64 = -180.0;
pub const LNG_MAX:          f64 =  180.0;
pub const RADIUS_MIN_M:     f64 = 100.0;
pub const RADIUS_MAX_M:     f64 = 50_000.0;
pub const RADIUS_DEFAULT_M: f64 = 5_000.0;

// ── Rate limiting ─────────────────────────────────────────────────────────────
pub const AUTH_RATE_LIMIT_PER_MIN:         u32 = 10;
pub const PUBLIC_WRITE_RATE_LIMIT_PER_MIN: u32 = 30;

// ── Database ──────────────────────────────────────────────────────────────────
pub const DB_MAX_CONNECTIONS: u32 = 20;

// ── Server ────────────────────────────────────────────────────────────────────
pub const SERVER_BIND_ADDR: &str = "0.0.0.0:3000";

// ── Roles ─────────────────────────────────────────────────────────────────────
pub const ROLE_ADMIN:    &str = "admin";
pub const ROLE_PROVIDER: &str = "provider";

// ── Permission codes (must match rows seeded in the init migration) ────────────
/// Provider permissions — automatically assigned on registration.
pub const PERM_PROVIDER_PROFILE_UPDATE: &str = "ProviderProfileUpdate";
pub const PERM_PROVIDER_LISTING_CREATE: &str = "ProviderListingCreate";
pub const PERM_PROVIDER_LISTING_UPDATE: &str = "ProviderListingUpdate";
pub const PERM_PROVIDER_LISTING_DELETE: &str = "ProviderListingDelete";

/// The full set granted to every newly registered provider.
pub const PROVIDER_DEFAULT_PERMISSIONS: &[&str] = &[
    PERM_PROVIDER_PROFILE_UPDATE,
    PERM_PROVIDER_LISTING_CREATE,
    PERM_PROVIDER_LISTING_UPDATE,
    PERM_PROVIDER_LISTING_DELETE,
];

// ── Business rules ────────────────────────────────────────────────────────────
pub const CATEGORY_TYPES:   &[&str] = &["service", "product"];
pub const INQUIRY_STATUSES: &[&str] = &["new", "contacted", "closed", "spam"];
