use chrono::{DateTime, Utc};
use domain::models::product::{AdminProduct, ProductView, ProviderProduct};
use serde::Serialize;
use uuid::Uuid;

// ── Location presets ──────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct LocationDto {
    pub id: i32,
    pub label: String,
    pub lat: f64,
    pub lng: f64,
}

#[derive(Serialize)]
pub struct AdminLocationDto {
    pub id: i32,
    pub label: String,
    pub lat: f64,
    pub lng: f64,
    pub display_order: i32,
    pub is_active: bool,
}

// ── Auth ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub username: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    /// The caller's role code, e.g. `"admin"` or `"provider"`.
    /// Used by frontend layouts to gate access without a separate role endpoint.
    pub role: String,
    pub created_on: String,
}

// ── Products (shared between public and provider views) ───────────────────────

#[derive(Serialize)]
pub struct ProductDto {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub category: String,
    pub vendor_name: String,
    pub vendor_phone: String,
    pub is_available: bool,
    pub created_on: DateTime<Utc>,
    /// Served at `/files/{id}` and `/files/{id}/thumb`.
    pub image_file_id: Option<Uuid>,
}

impl From<ProductView> for ProductDto {
    fn from(p: ProductView) -> Self {
        Self {
            id: p.id,
            name: p.name,
            description: p.description,
            price: p.price,
            unit: p.unit,
            category: p.category,
            vendor_name: p.vendor_name,
            vendor_phone: p.vendor_phone,
            is_available: p.is_available,
            created_on: p.created_on,
            image_file_id: p.image_file_id,
        }
    }
}

#[derive(Serialize)]
pub struct ProductBrowseResponse {
    pub items: Vec<ProductDto>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

// ── Agri-Connect / public provider views ─────────────────────────────────────

#[derive(Serialize)]
pub struct BusinessSearchDto {
    pub id: i32,
    pub name: String,
    pub phone: String,
    pub address: Option<String>,
    pub is_verified: bool,
    pub distance_m: f64,
    pub services: Vec<String>,
}

#[derive(Serialize)]
pub struct BusinessBrowseDto {
    pub id: i32,
    pub name: String,
    pub phone: String,
    pub address: Option<String>,
    pub is_verified: bool,
    pub services: Vec<String>,
    pub created_on: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct BrowseResponse {
    pub items: Vec<BusinessBrowseDto>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

#[derive(Serialize)]
pub struct ServiceDto {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
    pub category: String,
}

#[derive(Serialize)]
pub struct BusinessDetailDto {
    pub id: i32,
    pub name: String,
    pub phone: String,
    pub description: Option<String>,
    pub address: Option<String>,
    pub is_verified: bool,
    pub services: Vec<ServiceDto>,
}

#[derive(Serialize)]
pub struct ExploreData {
    pub featured_providers: Vec<BusinessBrowseDto>,
    pub featured_products: Vec<ProductDto>,
}

#[derive(Serialize)]
pub struct CategoryDto {
    pub id: i32,
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
}

// ── Provider dashboard ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ProviderProfileDto {
    pub id: i32,
    pub name: String,
    pub phone: String,
    pub email: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub is_verified: bool,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

#[derive(Serialize)]
pub struct ProviderProductDto {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub category: String,
    pub category_id: i32,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
    pub image_file_id: Option<Uuid>,
}

impl From<ProviderProduct> for ProviderProductDto {
    fn from(p: ProviderProduct) -> Self {
        Self {
            id: p.id,
            name: p.name,
            description: p.description,
            price: p.price,
            unit: p.unit,
            category: p.category,
            category_id: p.category_id,
            is_active: p.is_active,
            created_on: p.created_on,
            image_file_id: p.image_file_id,
        }
    }
}

// ── Admin ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AdminStatsDto {
    pub total_businesses: i64,
    pub verified_businesses: i64,
    pub active_products: i64,
    pub total_users: i64,
}

#[derive(Serialize)]
pub struct AdminProviderDto {
    pub id: i32,
    pub name: String,
    pub phone: String,
    pub address: Option<String>,
    pub is_verified: bool,
    pub is_active: bool,
    pub service_count: i64,
    pub product_count: i64,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
    pub linked_username: Option<String>,
    pub created_by_name: Option<String>,
    pub updated_by_name: Option<String>,
}

#[derive(Serialize)]
pub struct AdminServiceItemDto {
    pub id: i32,
    pub name: String,
    pub category: String,
    pub price_note: Option<String>,
    pub is_active: bool,
}

#[derive(Serialize)]
pub struct AdminProductItemDto {
    pub id: i32,
    pub name: String,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub category: String,
    pub is_active: bool,
}

#[derive(Serialize)]
pub struct AdminProviderDetailDto {
    pub id: i32,
    pub name: String,
    pub phone: String,
    pub email: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub is_verified: bool,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
    pub linked_username: Option<String>,
    pub services: Vec<AdminServiceItemDto>,
    pub products: Vec<AdminProductItemDto>,
}

#[derive(Serialize)]
pub struct AdminUserDto {
    pub id: i32,
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub role_code: String,
    pub role_name: String,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
    pub created_by_name: Option<String>,
    pub updated_by_name: Option<String>,
}

#[derive(Serialize)]
pub struct AdminUserDetailDto {
    pub id: i32,
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub role_code: String,
    pub role_name: String,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct VerifyToggleDto {
    pub id: i32,
    pub name: String,
    pub is_verified: bool,
}

#[derive(Serialize)]
pub struct AssignUserDto {
    pub business_id: i32,
    pub username: String,
}

#[derive(Serialize)]
pub struct AdminCategoryDto {
    pub id: i32,
    pub name: String,
    pub slug: String,
    pub category_type: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
    pub created_by_name: Option<String>,
    pub updated_by_name: Option<String>,
}

#[derive(Serialize)]
pub struct AdminInquiryDto {
    pub id: i32,
    pub business_id: i32,
    pub business_name: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub topic: String,
    pub status: String,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
    pub updated_by_name: Option<String>,
}

#[derive(Serialize)]
pub struct AdminReviewDto {
    pub id: i32,
    pub business_id: i32,
    pub business_name: String,
    pub reviewer_name: String,
    pub rating: i16,
    pub comment: Option<String>,
    pub is_visible: bool,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
    pub updated_by_name: Option<String>,
}

#[derive(Serialize)]
pub struct AdminPagedResponse<T: Serialize> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

// ── Admin products ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AdminProductDto {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub category: String,
    pub category_id: i32,
    pub business_id: i32,
    pub business_name: String,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
    pub image_file_id: Option<Uuid>,
}

impl From<AdminProduct> for AdminProductDto {
    fn from(p: AdminProduct) -> Self {
        Self {
            id: p.id,
            name: p.name,
            description: p.description,
            price: p.price,
            unit: p.unit,
            category: p.category,
            category_id: p.category_id,
            business_id: p.business_id,
            business_name: p.business_name,
            is_active: p.is_active,
            created_on: p.created_on,
            updated_on: p.updated_on,
            image_file_id: p.image_file_id,
        }
    }
}

// ── Admin services ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AdminServiceDto {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
    pub category: String,
    pub category_id: i32,
    pub business_id: i32,
    pub business_name: String,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
}

// ── Inquiry ───────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct InquiryCreatedDto {
    pub id: i32,
}

// ── Setup ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SetupStatusDto {
    pub database_connected: bool,
    /// True when demo listings are loaded (`businesses.is_demo`).
    pub is_seeded: bool,
    pub demo_business_count: i64,
    pub business_count: i64,
    pub user_count: i64,
}

// ── Roles ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct RoleDto {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
}

// ── Permissions ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct PermissionDto {
    pub id: i32,
    pub code: String,
    pub name: String,
}

impl PermissionDto {
    pub fn from_model(m: domain::models::permission::PermissionModel) -> Self {
        Self {
            id: m.id,
            code: m.code,
            name: m.name,
        }
    }
}

#[derive(Serialize)]
pub struct UserPermissionsDto {
    pub user_id: i32,
    pub permissions: Vec<PermissionDto>,
}
