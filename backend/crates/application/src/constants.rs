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

// ── Uploads ───────────────────────────────────────────────────────────────────
/// Used when UPLOAD_MAX_BYTES is unset. A phone photo is 2–4 MB.
pub const DEFAULT_UPLOAD_MAX_BYTES: usize = 5 * 1024 * 1024;
/// Checked from the header before decoding: a 50 KB PNG can claim 100 000 ×
/// 100 000 pixels and would take 40 GB to decode.
pub const IMAGE_MAX_PIXELS: u64 = 40_000_000;
/// Longest side of the stored image; enough for a phone screen at 2x.
pub const IMAGE_MAIN_MAX_SIDE: u32 = 1600;
/// Longest side of the list/card thumbnail.
pub const IMAGE_THUMB_MAX_SIDE: u32 = 400;
pub const IMAGE_JPEG_QUALITY: u8 = 80;

// ── Sessions ──────────────────────────────────────────────────────────────────
/// Used when REFRESH_TOKEN_DAYS is unset: a farmer opening the app weekly
/// stays signed in; one who stops for a month signs in again.
pub const REFRESH_TOKEN_DEFAULT_DAYS: i64 = 30;
/// A spent token presented again this soon is two tabs refreshing at once,
/// not theft: refuse it without revoking the family, or both tabs log out.
pub const REFRESH_REUSE_GRACE_SECS: i64 = 30;

// ── Catalogue fields (the VARCHAR sizes in the baseline schema) ────────────────
pub const CATEGORY_NAME_MAX: usize = 100;
pub const CATEGORY_SLUG_MAX: usize = 100;
pub const CATEGORY_ICON_MAX: usize = 100;
pub const CATEGORY_COLOR_MAX: usize = 20;
pub const CATEGORY_DESCRIPTION_MAX: usize = 300;
pub const LOCATION_LABEL_MAX: usize = 150;
