// ── Pagination ─────────────────────────────────────────────────────────────────
pub const PAGE_SIZE: i64 = 20;
pub const ADMIN_PAGE_SIZE: i64 = 25;
pub const MAX_PAGE: i64 = 10_000;

// ── Featured counts ────────────────────────────────────────────────────────────
pub const FEATURED_PROVIDERS_COUNT: i64 = 6;
pub const FEATURED_PRODUCTS_COUNT: i64 = 8;
/// Services on the home page and the near-me page before a position is
/// known: one per verified provider.
pub const FEATURED_SERVICES_COUNT: usize = 6;

// ── Authentication ─────────────────────────────────────────────────────────────
pub const JWT_EXPIRY_HOURS: i64 = 1;
pub const PASSWORD_RESET_EXPIRY_HOURS: i64 = 1;
pub const LOGIN_FAIL_MAX_ATTEMPTS: i32 = 5;
pub const LOGIN_LOCK_MINUTES: i64 = 15;

// ── Validation ─────────────────────────────────────────────────────────────────
pub const USERNAME_MIN_LEN: usize = 3;
/// Long enough for any real login name; the column holds 100.
pub const USERNAME_MAX_LEN: usize = 50;
/// The `users.full_name` column.
pub const FULL_NAME_MAX: usize = 200;
/// `users.email` is VARCHAR(255).
pub const EMAIL_MAX: usize = 255;
/// Counted in characters, so a Vietnamese password is not penalised for its
/// multi-byte letters.
pub const PASSWORD_MIN_CHARS: usize = 8;
/// bcrypt silently ignores everything after byte 72, so a longer password
/// would look accepted while only its prefix counts. Reject instead.
pub const PASSWORD_MAX_BYTES: usize = 72;
pub const REQUESTER_NAME_MAX_LEN: usize = 100;
/// The `inquiries.topic` column. The contact form's topic is a service name,
/// so this must be at least `SERVICE_NAME_MAX` (asserted at compile time).
pub const TOPIC_MAX_LEN: usize = 300;
pub const NOTE_MAX_LEN: usize = 1000;

// ── Geography ─────────────────────────────────────────────────────────────────
pub const LAT_MIN: f64 = -90.0;
pub const LAT_MAX: f64 = 90.0;
pub const LNG_MIN: f64 = -180.0;
pub const LNG_MAX: f64 = 180.0;
pub const RADIUS_MIN_M: f64 = 100.0;
pub const RADIUS_MAX_M: f64 = 50_000.0;
/// A farm visit by motorbike; the near-me page and the JSON API share it.
pub const RADIUS_DEFAULT_M: f64 = 10_000.0;
/// Most results one near-me search returns (providers or services), nearest
/// first: enough for a district without a page that never ends.
pub const GEO_RESULTS_MAX: i64 = 50;
/// The window the call counts cover (admin dashboard, provider dashboard).
pub const CALL_STATS_DAYS: i64 = 30;
/// How many weeks of leads (calls, Zalo chats, inquiries) the admin dashboard
/// shows, newest first: enough to see whether a field visit took hold.
pub const LEAD_WEEKS: i64 = 8;

// ── Demand report ─────────────────────────────────────────────────────────────
/// Vietnam is UTC+7 all year (no daylight saving since 1975), so a fixed
/// offset is exact: report weeks, dates shown on pages.
pub const VN_UTC_OFFSET_SECS: i32 = 7 * 3600;
/// How many whole weeks the demand report covers, ending last Sunday.
pub const DEMAND_WEEKS: usize = 8;
/// A figure under this (and above zero) is shown as "fewer than": it could
/// point at one household.
pub const DEMAND_REPORT_MIN_COUNT: i64 = 5;
/// A business counts under the nearest location preset within this reach.
pub const DEMAND_AREA_RADIUS_M: f64 = RADIUS_DEFAULT_M;
/// An area is steady when every one of the report's weeks has at least this
/// many leads: ready to show partners.
pub const STEADY_WEEKLY_LEADS: i64 = 20;
/// A stored search keeps at most this many characters of its words.
pub const SEARCH_QUERY_MAX: usize = 100;
/// How many searches that found nothing the report lists, most asked first.
pub const UNMET_SEARCHES_SHOWN: usize = 10;

// ── Partners ──────────────────────────────────────────────────────────────────
pub const PARTNER_NAME_MAX: usize = 200;
pub const CONTRACT_NOTE_MAX: usize = 500;
/// How many sponsored services a near-me page shows above its results.
pub const SPONSORED_SHOWN: usize = 2;

// ── Reviews ───────────────────────────────────────────────────────────────────
pub const RATING_MIN: i16 = 1;
pub const RATING_MAX: i16 = 5;
pub const REVIEW_COMMENT_MAX: usize = 500;
pub const REVIEW_REPLY_MAX: usize = 500;
/// How many shown reviews a provider's page lists, newest first.
pub const PUBLIC_REVIEWS_SHOWN: i64 = 20;

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
/// Numbered slug candidates (`base`, `base-2`, …) checked per query while
/// looking for a free one.
pub const SLUG_CANDIDATES_PER_QUERY: u32 = 10;
pub const LOCATION_LABEL_MAX: usize = 150;
pub const SERVICE_NAME_MAX: usize = 300;
pub const SERVICE_PRICE_NOTE_MAX: usize = 300;
/// `services.description` is TEXT; this keeps a listing readable on a phone.
pub const SERVICE_DESCRIPTION_MAX: usize = 2000;
/// How many services one business may list (hidden ones count), so deleting
/// and re-adding cannot flood the directory.
pub const SERVICES_PER_BUSINESS_MAX: u64 = 20;
pub const PRODUCT_NAME_MAX: usize = 300;
pub const PRODUCT_UNIT_MAX: usize = 50;
pub const PRODUCT_DESCRIPTION_MAX: usize = 2000;
pub const BUSINESS_NAME_MAX: usize = 200;
pub const BUSINESS_ADDRESS_MAX: usize = 500;
pub const BUSINESS_DESCRIPTION_MAX: usize = 2000;
/// `audit_log.reason` is TEXT; an admin's "why" is a sentence, not an essay.
pub const AUDIT_REASON_MAX: usize = 500;

// Every service name must fit as a contact topic.
const _: () = assert!(TOPIC_MAX_LEN >= SERVICE_NAME_MAX);
