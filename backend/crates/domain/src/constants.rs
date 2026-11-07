// ── Roles ─────────────────────────────────────────────────────────────────────
// Codes stored in `roles.code`. The role catalogue in `application` seeds them.
pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_PROVIDER: &str = "provider";
pub const ROLE_PARTNER: &str = "partner";

/// What kind of company a partner is. Keep in sync with the `partners.kind` CHECK.
pub const PARTNER_KINDS: &[&str] = &["brand", "dealer", "cooperative", "exporter"];

/// What a crop is, for the price page's sections. Keep in sync with the
/// `crops.kind` CHECK.
pub const CROP_KIND_RICE: &str = "rice";
pub const CROP_KIND_FRUIT: &str = "fruit";
pub const CROP_KINDS: &[&str] = &[CROP_KIND_RICE, CROP_KIND_FRUIT];

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

/// A job post's state: open (providers see it), closed (done or no longer
/// needed), spam (junk; never shown). Keep in sync with the `job_posts`
/// CHECK.
pub const JOB_STATUS_OPEN: &str = "open";
pub const JOB_STATUSES: &[&str] = &[JOB_STATUS_OPEN, "closed", "spam"];
