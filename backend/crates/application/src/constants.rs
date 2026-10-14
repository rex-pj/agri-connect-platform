// ── Pagination ─────────────────────────────────────────────────────────────────
pub const PAGE_SIZE: i64 = 20;
pub const ADMIN_PAGE_SIZE: i64 = 25;
pub const MAX_PAGE: i64 = 10_000;

// ── Featured counts ────────────────────────────────────────────────────────────
pub const FEATURED_PROVIDERS_COUNT: i64 = 6;
pub const FEATURED_PRODUCTS_COUNT: i64 = 8;

// ── Authentication ─────────────────────────────────────────────────────────────
pub const JWT_EXPIRY_HOURS: i64 = 1;
pub const PASSWORD_RESET_EXPIRY_HOURS: i64 = 1;
pub const LOGIN_FAIL_MAX_ATTEMPTS: i32 = 5;
pub const LOGIN_LOCK_MINUTES: i64 = 15;

// ── Validation ─────────────────────────────────────────────────────────────────
pub const USERNAME_MIN_LEN: usize = 3;
/// Counted in characters, so a Vietnamese password is not penalised for its
/// multi-byte letters.
pub const PASSWORD_MIN_CHARS: usize = 8;
/// bcrypt silently ignores everything after byte 72, so a longer password
/// would look accepted while only its prefix counts. Reject instead.
pub const PASSWORD_MAX_BYTES: usize = 72;
pub const REQUESTER_NAME_MAX_LEN: usize = 100;
pub const TOPIC_MAX_LEN: usize = 255;
pub const NOTE_MAX_LEN: usize = 1000;

// ── Geography ─────────────────────────────────────────────────────────────────
pub const LAT_MIN: f64 = -90.0;
pub const LAT_MAX: f64 = 90.0;
pub const LNG_MIN: f64 = -180.0;
pub const LNG_MAX: f64 = 180.0;
pub const RADIUS_MIN_M: f64 = 100.0;
pub const RADIUS_MAX_M: f64 = 50_000.0;
pub const RADIUS_DEFAULT_M: f64 = 5_000.0;
