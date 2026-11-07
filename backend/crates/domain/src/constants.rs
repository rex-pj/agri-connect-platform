// ── Roles ─────────────────────────────────────────────────────────────────────
// Codes stored in `roles.code`. The role catalogue in `application` seeds them.
pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_PROVIDER: &str = "provider";

// ── Business rules ────────────────────────────────────────────────────────────
// Must match the CHECK constraints in the baseline migration; a value added here
// but not there fails at insert time.
pub const CATEGORY_TYPE_SERVICE: &str = "service";
pub const CATEGORY_TYPE_PRODUCT: &str = "product";
pub const CATEGORY_TYPES: &[&str] = &[CATEGORY_TYPE_SERVICE, CATEGORY_TYPE_PRODUCT];
/// An inquiry marked as junk: never counted as a lead.
pub const INQUIRY_STATUS_SPAM: &str = "spam";
pub const INQUIRY_STATUS_CONTACTED: &str = "contacted";
pub const INQUIRY_STATUS_CLOSED: &str = "closed";
pub const INQUIRY_STATUSES: &[&str] = &[
    "new",
    INQUIRY_STATUS_CONTACTED,
    INQUIRY_STATUS_CLOSED,
    INQUIRY_STATUS_SPAM,
];
/// An inquiry the provider answered (contacted, or done): the farmer behind
/// it may review that provider. A new or spam one may not.
pub const INQUIRY_STATUSES_REVIEWABLE: &[&str] = &[INQUIRY_STATUS_CONTACTED, INQUIRY_STATUS_CLOSED];

/// What a public call button was on (`data-call-kind` in the templates).
pub const CALL_KIND_SERVICE: &str = "service";
pub const CALL_KIND_PRODUCT: &str = "product";
pub const CALL_KIND_PROVIDER: &str = "provider";

/// How the farmer reached out from that button (`data-call-channel`): a phone
/// call or a Zalo chat. Keep in sync with the `call_clicks.channel` CHECK.
pub const CALL_CHANNEL_PHONE: &str = "call";
pub const CALL_CHANNEL_ZALO: &str = "zalo";
pub const CALL_CHANNELS: &[&str] = &[CALL_CHANNEL_PHONE, CALL_CHANNEL_ZALO];
