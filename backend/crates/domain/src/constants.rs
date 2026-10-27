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
pub const INQUIRY_STATUSES: &[&str] = &["new", "contacted", "closed", "spam"];

/// What a public call button was on (`data-call-kind` in the templates).
pub const CALL_KIND_SERVICE: &str = "service";
pub const CALL_KIND_PRODUCT: &str = "product";
pub const CALL_KIND_PROVIDER: &str = "provider";
