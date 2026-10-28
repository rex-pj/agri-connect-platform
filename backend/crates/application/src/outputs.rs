use chrono::{DateTime, Utc};
use domain::models::{
    inquiry::InquiryForOwner,
    product::{AdminProduct, ProductView, ProviderProduct},
    service::{AdminService, ProviderService, PublicService},
};
use serde::Serialize;
use uuid::Uuid;

// ── Location presets ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
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
    /// For starting a browser session; not part of the JSON API.
    #[serde(skip_serializing)]
    pub user_id: i32,
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
    pub category_icon: Option<String>,
    pub vendor_name: String,
    pub vendor_phone: String,
    /// The seller carries the verified badge.
    pub vendor_verified: bool,
    pub business_id: i32,
    pub created_on: DateTime<Utc>,
    /// Served at `/files/{id}` and `/files/{id}/thumb`.
    pub image_file_id: Option<Uuid>,
    /// Metres from the searched point; only on a near-me search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_m: Option<f64>,
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
            category_icon: p.category_icon,
            vendor_name: p.vendor_name,
            vendor_phone: p.vendor_phone,
            vendor_verified: p.vendor_verified,
            business_id: p.business_id,
            created_on: p.created_on,
            image_file_id: p.image_file_id,
            distance_m: p.distance_m,
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

/// A service as farmers see it: in the listing, near-me results and on its
/// own page.
#[derive(Serialize)]
pub struct PublicServiceDto {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
    pub category_id: i32,
    pub category: String,
    pub category_slug: String,
    pub category_icon: Option<String>,
    pub image_file_id: Option<Uuid>,
    pub business_id: i32,
    pub business_name: String,
    pub business_phone: String,
    pub business_address: Option<String>,
    pub business_verified: bool,
    pub created_on: DateTime<Utc>,
    /// Metres from the searched point; only on a near-me search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_m: Option<f64>,
}

impl From<PublicService> for PublicServiceDto {
    fn from(s: PublicService) -> Self {
        Self {
            id: s.id,
            name: s.name,
            description: s.description,
            price_note: s.price_note,
            category_id: s.category_id,
            category: s.category,
            category_slug: s.category_slug,
            category_icon: s.category_icon,
            image_file_id: s.image_file_id,
            business_id: s.business_id,
            business_name: s.business_name,
            business_phone: s.business_phone,
            business_address: s.business_address,
            business_verified: s.business_verified,
            created_on: s.created_on,
            distance_m: s.distance_m,
        }
    }
}

#[derive(Serialize)]
pub struct ServiceBrowseResponse {
    pub items: Vec<PublicServiceDto>,
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
    pub category_icon: Option<String>,
    pub image_file_id: Option<Uuid>,
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
    /// One service per verified provider, those with a photo first.
    pub featured_services: Vec<PublicServiceDto>,
    pub featured_providers: Vec<BusinessBrowseDto>,
    pub featured_products: Vec<ProductDto>,
}

#[derive(Debug, Clone, Serialize)]
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

/// A contact request in the provider's own inbox.
#[derive(Debug, Serialize)]
pub struct OwnInquiryDto {
    pub id: i32,
    pub service_name: Option<String>,
    pub topic: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub note: Option<String>,
    pub status: String,
    pub created_on: DateTime<Utc>,
}

impl From<InquiryForOwner> for OwnInquiryDto {
    fn from(i: InquiryForOwner) -> Self {
        Self {
            id: i.id,
            service_name: i.service_name,
            topic: i.topic,
            requester_name: i.requester_name,
            requester_phone: i.requester_phone,
            note: i.note,
            status: i.status,
            created_on: i.created_on,
        }
    }
}

/// One page of the provider's inbox, and how many still wait for a call.
#[derive(Debug, Serialize)]
pub struct OwnInquiriesResponse {
    pub items: Vec<OwnInquiryDto>,
    pub total: i64,
    /// Still `new`: nobody has called back yet.
    pub new_count: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

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
    /// Taps on public call buttons in the last `CALL_STATS_DAYS` days.
    pub recent_calls: i64,
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
pub struct VerificationDto {
    pub id: i32,
    pub name: String,
    pub is_verified: bool,
}

/// A provider turned on or off by the admin.
#[derive(Serialize)]
pub struct ProviderStatusDto {
    pub id: i32,
    pub name: String,
    pub is_active: bool,
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

/// A free slug for what was typed in a category form (its name or its slug).
#[derive(Serialize)]
pub struct SlugSuggestionDto {
    /// Empty when the text has nothing a slug can keep.
    pub slug: String,
    /// The text's own slug belongs to another category: `slug` is numbered.
    pub taken: bool,
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
    pub image_file_id: Option<Uuid>,
    /// Listed by the provider themselves, not by an admin for them.
    pub added_by_owner: bool,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
}

impl From<AdminService> for AdminServiceDto {
    fn from(s: AdminService) -> Self {
        Self {
            added_by_owner: s.created_by.is_some() && s.created_by == s.owner_id,
            id: s.id,
            name: s.name,
            description: s.description,
            price_note: s.price_note,
            category: s.category,
            category_id: s.category_id,
            business_id: s.business_id,
            business_name: s.business_name,
            is_active: s.is_active,
            image_file_id: s.image_file_id,
            created_on: s.created_on,
            updated_on: s.updated_on,
        }
    }
}

// ── Provider services ─────────────────────────────────────────────────────────

/// One of the provider's own services, hidden ones included.
#[derive(Serialize)]
pub struct ProviderServiceDto {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
    pub category: String,
    pub category_id: i32,
    pub category_icon: Option<String>,
    /// `false` when an admin hid it; only an admin can show it again.
    pub is_active: bool,
    pub image_file_id: Option<Uuid>,
    pub created_on: DateTime<Utc>,
}

impl From<ProviderService> for ProviderServiceDto {
    fn from(s: ProviderService) -> Self {
        Self {
            id: s.id,
            name: s.name,
            description: s.description,
            price_note: s.price_note,
            category: s.category,
            category_id: s.category_id,
            category_icon: s.category_icon,
            is_active: s.is_active,
            image_file_id: s.image_file_id,
            created_on: s.created_on,
        }
    }
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

impl From<domain::models::role::RoleModel> for RoleDto {
    fn from(r: domain::models::role::RoleModel) -> Self {
        Self {
            id: r.id,
            code: r.code,
            name: r.name,
            description: r.description,
            is_active: r.is_active,
        }
    }
}

/// One permission, and whether the role in question grants it.
#[derive(Debug, Serialize)]
pub struct GrantDto {
    pub code: String,
    pub name: String,
    pub granted: bool,
}

#[derive(Serialize)]
pub struct RoleGrantsDto {
    pub role: RoleDto,
    pub permissions: Vec<GrantDto>,
}

#[derive(Serialize)]
pub struct PermissionMatrixRowDto {
    pub code: String,
    pub name: String,
    /// One entry per role, in the order of `PermissionMatrixDto::roles`.
    pub granted: Vec<bool>,
}

#[derive(Serialize)]
pub struct PermissionMatrixDto {
    pub roles: Vec<RoleDto>,
    pub rows: Vec<PermissionMatrixRowDto>,
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
