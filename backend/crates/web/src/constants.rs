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
pub const AUTH_RATE_LIMIT_PER_MIN:         NonZeroU32 = per_minute(10);
pub const PUBLIC_WRITE_RATE_LIMIT_PER_MIN: NonZeroU32 = per_minute(30);

// ── Database ──────────────────────────────────────────────────────────────────
pub const DB_MAX_CONNECTIONS: u32 = 20;

// ── Server ────────────────────────────────────────────────────────────────────
pub const SERVER_BIND_ADDR: &str = "0.0.0.0:3000";
