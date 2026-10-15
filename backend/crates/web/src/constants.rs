use std::num::NonZeroU32;

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
/// Generous: an admin clearing a moderation queue clicks fast. It exists to
/// stop a stolen session or a runaway script, not a person.
pub const MEMBER_WRITE_RATE_LIMIT_PER_MIN: NonZeroU32 = per_minute(120);

// ── Database ──────────────────────────────────────────────────────────────────
pub const DB_MAX_CONNECTIONS: u32 = 20;

// ── Server ────────────────────────────────────────────────────────────────────
/// Used when BIND_ADDR is unset. 0.0.0.0 so the container port mapping works.
pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:3000";

/// Used when APP_URL is unset: Rust is the front door on :3000 in development.
pub const DEFAULT_APP_URL: &str = "http://localhost:3000";

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
/// Featured providers under the near-me banner.
pub const HOME_PROVIDER_COUNT: usize = 3;

// ── Pages ─────────────────────────────────────────────────────────────────────
/// Search engines show about this much of a meta description.
pub const META_DESCRIPTION_MAX_CHARS: usize = 150;

// ── Search page ───────────────────────────────────────────────────────────────
/// Results of each kind shown on /search; "Xem thêm" leads to the full list.
pub const SEARCH_PREVIEW_COUNT: usize = 6;
