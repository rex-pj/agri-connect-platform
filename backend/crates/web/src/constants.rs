use std::num::NonZeroU32;

use application::constants::RADIUS_DEFAULT_M;

const fn per_minute(n: u32) -> NonZeroU32 {
    // A zero quota would block every request; fall back to 1/min so a typo
    // here throttles hard instead of failing to compile in a const context.
    match NonZeroU32::new(n) {
        Some(v) => v,
        None => NonZeroU32::MIN,
    }
}

// ── Rate limiting (per client IP, in-process) ─────────────────────────────────
pub const AUTH_RATE_LIMIT_PER_MIN: NonZeroU32 = per_minute(10);
pub const PUBLIC_WRITE_RATE_LIMIT_PER_MIN: NonZeroU32 = per_minute(30);
/// Searches (text and near-me, pages and JSON): each is a query over the
/// listings. Generous, because one carrier address is many farmers; it stops
/// a script, not a village.
pub const PUBLIC_SEARCH_RATE_LIMIT_PER_MIN: NonZeroU32 = per_minute(240);
/// An IPv6 client is its /64: one subscriber holds the whole prefix, and a
/// budget per address would be no budget at all.
pub const RATE_LIMIT_IPV6_PREFIX_BITS: u32 = 64;
/// The longest `x-request-id` a client may bring; a UUID is 36.
pub const REQUEST_ID_MAX_LEN: usize = 64;
/// How long a request may run before it is given up on (uploads excepted).
pub const REQUEST_TIMEOUT_SECS: u64 = 30;
/// Generous: an admin clearing a moderation queue clicks fast. It exists to
/// stop a stolen session or a runaway script, not a person.
pub const MEMBER_WRITE_RATE_LIMIT_PER_MIN: NonZeroU32 = per_minute(120);

// ── Contact ───────────────────────────────────────────────────────────────────
/// A Zalo chat with a Vietnamese phone number is this followed by the number
/// (the `zalo` template filter): every provider is reachable there by phone.
pub const ZALO_CHAT_URL: &str = "https://zalo.me/";
/// A map at one point is this followed by `lat,lng`: the admin checks where
/// a provider was placed (its ward, the near-me page) against the map.
pub const MAP_POINT_URL: &str = "https://www.google.com/maps?q=";

// ── Database ──────────────────────────────────────────────────────────────────
/// Connections in the pool when `DB_MAX_CONNECTIONS` is unset. A page takes a
/// few at once (its lists side by side), so this is a handful of pages in
/// flight, not twenty.
pub const DB_MAX_CONNECTIONS: u32 = 20;
/// How long a request waits for a free connection before it fails, in
/// seconds: a full pool answers "try again" at once instead of queueing
/// every visitor behind the slow ones.
pub const DB_ACQUIRE_TIMEOUT_SECS: u64 = 5;
/// The longest one statement may run when `DB_STATEMENT_TIMEOUT_SECS` is
/// unset, in seconds. Under `REQUEST_TIMEOUT_SECS`: a query whose request
/// was given up on must not keep its connection busy. Migrations run on
/// their own connection without it.
pub const DB_STATEMENT_TIMEOUT_SECS: u64 = 20;

// ── Server ────────────────────────────────────────────────────────────────────
/// Used when BIND_ADDR is unset. 0.0.0.0 so the container port mapping works.
pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:5173";

/// Used when APP_URL is unset: Rust is the front door on :5173 in development.
pub const DEFAULT_APP_URL: &str = "http://localhost:5173";

// ── Uploads ───────────────────────────────────────────────────────────────────
/// Decoding is the expensive part of an upload; this budget is per client and
/// separate from other writes, so image uploads cannot starve form saves.
pub const UPLOAD_RATE_LIMIT_PER_MIN: NonZeroU32 = per_minute(20);
/// Multipart boundaries and part headers on top of the file itself. Without
/// it, a file of exactly UPLOAD_MAX_BYTES is cut off by the body limit.
pub const MULTIPART_OVERHEAD_BYTES: usize = 64 * 1024;
/// The multipart field the image arrives in.
pub const IMAGE_FIELD: &str = "image";
/// An id is never reused (a new image gets a new id), so a cached copy can
/// never go stale.
pub const FILE_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
/// Used when GCS_BUCKET and UPLOAD_DIR are both unset.
pub const DEFAULT_UPLOAD_DIR: &str = "./data/uploads";

// ── Home page ─────────────────────────────────────────────────────────────────
/// Featured products shown: one row on a wide screen, three on a phone.
pub const HOME_PRODUCT_COUNT: usize = 6;
/// Category tiles of each kind before its "all" tile on a wide screen; a
/// phone shows one fewer, so each kind is one row of four there.
pub const HOME_CATEGORY_TILES: usize = 4;
/// How many crop prices the home page shows (each crop's newest).
pub const HOME_PRICES_SHOWN: usize = 4;

// ── Admin dashboard ───────────────────────────────────────────────────────────
/// Areas of the demand report listed as nearest to steady (most weeks at
/// the bar first).
pub const DASHBOARD_NEAR_AREAS: usize = 5;

// ── Pages ─────────────────────────────────────────────────────────────────────
/// Search engines show about this much of a meta description.
pub const META_DESCRIPTION_MAX_CHARS: usize = 150;

// ── Search page ───────────────────────────────────────────────────────────────
/// Results of each kind shown on /search; "Xem thêm" leads to the full list.
pub const SEARCH_PREVIEW_COUNT: usize = 6;

// ── Near-me page ──────────────────────────────────────────────────────────────
/// The radius choices on /explore, in metres. Each is inside the use case's
/// 100 m – 50 km bounds.
pub const EXPLORE_RADII_M: [i64; 4] = [5_000, 10_000, 25_000, 50_000];
/// The use case's default (the onboarding promises "trong 10 km"), as a
/// whole number of metres for the radius links. Must be one of the choices.
pub const EXPLORE_DEFAULT_RADIUS_M: i64 = RADIUS_DEFAULT_M as i64;
const _: () = assert!(EXPLORE_RADII_M[1] == EXPLORE_DEFAULT_RADIUS_M);

/// Metres in a kilometre: radii are kept in metres, shown in whole km.
pub use application::constants::METRES_PER_KM;

/// A distance in metres as whole kilometres, as the pages show it.
pub const fn km(metres: i64) -> i64 {
    metres / METRES_PER_KM
}

// ── Server ────────────────────────────────────────────────────────────────────
/// The SMTP port when `SMTP_PORT` is unset (submission with STARTTLS).
pub const SMTP_PORT_DEFAULT: u16 = 587;
/// How often expired refresh tokens are deleted, in seconds.
pub const SESSION_PURGE_EVERY_SECS: u64 = 3_600;
/// How often taps and searches older than `EVENT_KEEP_WEEKS` are counted
/// into the weekly totals and deleted, and job posts older than
/// `JOB_POST_KEEP_DAYS` deleted, in seconds. A day is soon enough, and the
/// first run is at startup.
pub const EVENT_ROLL_UP_EVERY_SECS: u64 = 86_400;
/// How often the rate limiters forget idle clients, and the repeat filters
/// the taps and searches past their window, in seconds.
pub const RATE_LIMIT_PRUNE_EVERY_SECS: u64 = 300;
/// How long the `healthcheck` command waits for the server, in seconds.
pub const HEALTHCHECK_TIMEOUT_SECS: u64 = 5;

/// The Zalo Graph endpoint that converts a Mini App's location or phone
/// token (with the app's secret key) into the data it stands for.
pub const ZALO_GRAPH_URL: &str = "https://graph.zalo.me/v2.0/me/info";
/// How long that call may take, in seconds, before the farmer is told to
/// try again (a token expires two minutes after it was issued).
pub const ZALO_TIMEOUT_SECS: u64 = 10;

// ── `agri-connect demo-data` (the demo data at volume) ────────────────────────
/// Providers generated when `--providers` is not given: the size the
/// platform is tried and timed at (about 5 000 rows of each kind).
pub const DEMO_VOLUME_PROVIDERS_DEFAULT: u32 = 5_000;
/// The most `--providers` takes: everything is written in one transaction.
pub const DEMO_VOLUME_PROVIDERS_MAX: u32 = 50_000;
/// The seed when `--seed` is not given: the same rows on every machine.
pub const DEMO_VOLUME_SEED_DEFAULT: u64 = 2_026;
