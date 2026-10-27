// Input parameter types passed from handlers into use cases.
// These are internal application-layer types — not HTTP request structs.

use domain::models::business::ProviderSort;

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
    pub sort: ProviderSort,
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

/// A provider's own service, for create and update alike.
pub struct ServiceInput {
    pub category_id: i32,
    /// Blank takes the category's name.
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
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

/// A tap on a public call button: which kind of card or page (`service`,
/// `product`, `provider`) and that record's id. The business it rings is
/// looked up from the record, never taken from the request.
pub struct CallInput {
    pub kind: String,
    pub id: i32,
}
