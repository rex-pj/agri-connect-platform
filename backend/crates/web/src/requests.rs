//! HTTP request shapes. Field rules live in the use cases, not here, so the
//! JSON API and the server-rendered forms enforce exactly the same ones.

use application::inputs::ServiceInput;
use serde::Deserialize;

// ── Auth ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct CreateAdminRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub full_name: Option<String>,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub full_name: Option<String>,
    pub business_name: String,
    pub business_phone: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    pub new_password: String,
}

// ── Admin ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AdminPageQuery {
    pub page: Option<i64>,
}

#[derive(Deserialize)]
pub struct AssignUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct UpdateUserRoleRequest {
    pub role_code: String,
}

#[derive(Deserialize)]
pub struct ToggleActiveRequest {
    pub is_active: bool,
    pub reason: Option<String>,
}

/// Optional JSON body for DELETE endpoints that support an audit reason.
#[derive(Deserialize, Default)]
pub struct DeleteWithReasonBody {
    pub reason: Option<String>,
}

/// Optional JSON body for verification toggle (PATCH with no other payload).
#[derive(Deserialize, Default)]
pub struct SetVerificationRequest {
    pub is_verified: bool,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateCategoryRequest {
    pub name: String,
    pub slug: String,
    pub category_type: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
}

#[derive(Deserialize)]
pub struct UpdateCategoryRequest {
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
}

#[derive(Deserialize)]
pub struct UpdateInquiryStatusRequest {
    pub status: String,
}

#[derive(Deserialize)]
pub struct CreateLocationRequest {
    pub label: String,
    pub lat: f64,
    pub lng: f64,
    pub display_order: i32,
}

#[derive(Deserialize)]
pub struct UpdateLocationRequest {
    pub label: String,
    pub lat: f64,
    pub lng: f64,
    pub display_order: i32,
}

#[derive(Deserialize)]
pub struct ToggleVisibleRequest {
    pub is_visible: bool,
    pub reason: Option<String>,
}

// ── Provider dashboard ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub name: String,
    pub phone: String,
    pub description: Option<String>,
    pub address: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

#[derive(Deserialize)]
pub struct CreateProductRequest {
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateProductRequest {
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
}

/// POST and PUT /api/provider/services: one of the caller's own services.
#[derive(Deserialize)]
pub struct OwnServiceRequest {
    pub category_id: i32,
    /// Missing or blank takes the category's name.
    #[serde(default)]
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
}

impl OwnServiceRequest {
    pub fn into_input(self) -> ServiceInput {
        ServiceInput {
            category_id: self.category_id,
            name: self.name,
            description: self.description,
            price_note: self.price_note,
        }
    }
}

// ── Admin products & services ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AdminListQuery {
    pub page: Option<i64>,
    pub provider_id: Option<i32>,
}

#[derive(Deserialize)]
pub struct AdminCreateServiceRequest {
    pub business_id: i32,
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
}

#[derive(Deserialize)]
pub struct AdminUpdateServiceRequest {
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
}

// ── Products ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ProductBrowseQuery {
    pub category_id: Option<i32>,
    pub name: Option<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
    /// Accepted values: "newest" (default) | "price_asc" | "price_desc"
    pub sort: Option<String>,
    pub page: Option<i64>,
}

// ── Explore ───────────────────────────────────────────────────────────────────

/// Combined query for GET /api/explore/providers and /api/explore/services.
/// When `lat` and `lng` are present the handler runs a geo-search;
/// otherwise it runs a paginated browse.
#[derive(Deserialize)]
pub struct ProvidersQuery {
    // ── geo-search fields ──────────────────────────────────────────────────
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub radius: Option<f64>,
    // ── shared ────────────────────────────────────────────────────────────
    /// A service *category* id: providers offering a live service in it, or
    /// the services in it.
    /// `category_id` is the clear name; `service_id` is kept for existing
    /// clients (it is not a `services.id`, unlike the inquiry's).
    #[serde(alias = "category_id")]
    pub service_id: Option<i32>,
    // ── browse fields ─────────────────────────────────────────────────────
    pub name: Option<String>,
    /// Accepted values: "verified_first" (default) | "newest" | "name_asc"
    pub sort: Option<String>,
    pub page: Option<i64>,
}

#[derive(Deserialize)]
pub struct CategoriesQuery {
    #[serde(rename = "type")]
    pub cat_type: Option<String>,
}

#[derive(Deserialize)]
pub struct InquiryRequest {
    pub business_id: i32,
    pub topic: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub note: Option<String>,
    pub service_id: Option<i32>,
}

/// POST /api/explore/calls: a tap on a public call button (sent by the
/// `callBeacon` Alpine component).
#[derive(Deserialize)]
pub struct CallRequest {
    pub kind: String,
    pub id: i32,
}
