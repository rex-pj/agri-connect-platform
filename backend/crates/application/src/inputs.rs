// Input parameter types passed from handlers into use cases.
// These are internal application-layer types — not HTTP request structs.

// ── Agri-connect ──────────────────────────────────────────────────────────────

pub struct GeoSearchParams {
    pub lat: f64,
    pub lng: f64,
    pub radius: f64,
    pub service_id: Option<i32>,
}

pub struct BrowseParams {
    pub service_id: Option<i32>,
    pub name: Option<String>,
    /// "verified_first" (default) | "newest" | "name_asc"
    pub sort: Option<String>,
    pub page: i64,
}

pub struct ContactInput {
    pub business_id: i32,
    pub topic: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub note: Option<String>,
    pub service_id: Option<i32>,
}

// ── Provider dashboard ────────────────────────────────────────────────────────

pub struct UpdateProfileInput {
    pub name: String,
    pub phone: String,
    pub description: Option<String>,
    pub address: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

pub struct CreateProductInput {
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
}

pub struct UpdateProductInput {
    pub product_id: i32,
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
}

// ── Admin catalogue ───────────────────────────────────────────────────────────

/// Editable category fields, shared by create and update.
pub struct CategoryInput {
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
}
