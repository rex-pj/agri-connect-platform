use serde::Deserialize;
use validator::Validate;

use crate::{
    constants::{LAT_MAX, LAT_MIN, LNG_MAX, LNG_MIN, RADIUS_MAX_M, RADIUS_MIN_M},
    validators::validate_vn_phone,
};

// ── Auth ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct CreateAdminRequest {
    pub username:  String,
    pub email:     String,
    pub password:  String,
    pub full_name: Option<String>,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username:       String,
    pub email:          String,
    pub password:       String,
    pub full_name:      Option<String>,
    pub business_name:  String,
    pub business_phone: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password:     String,
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
    pub email:    String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct UpdateUserRoleRequest {
    pub role_code: String,
}

#[derive(Deserialize)]
pub struct ToggleActiveRequest {
    pub is_active: bool,
}

#[derive(Deserialize)]
pub struct CreateCategoryRequest {
    pub name:          String,
    pub slug:          String,
    pub category_type: String,
    pub icon:          Option<String>,
    pub display_order: i16,
}

#[derive(Deserialize)]
pub struct UpdateCategoryRequest {
    pub name:          String,
    pub slug:          String,
    pub icon:          Option<String>,
    pub display_order: i16,
}

#[derive(Deserialize)]
pub struct UpdateInquiryStatusRequest {
    pub status: String,
}

#[derive(Deserialize)]
pub struct ToggleVisibleRequest {
    pub is_visible: bool,
}

// ── Provider dashboard ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub name:        String,
    pub phone:       String,
    pub description: Option<String>,
    pub address:     Option<String>,
    pub lat:         Option<f64>,
    pub lng:         Option<f64>,
}

#[derive(Deserialize)]
pub struct CreateProductRequest {
    pub category_id: i32,
    pub name:        String,
    pub description: Option<String>,
    pub price:       Option<i64>,
    pub unit:        Option<String>,
    pub image_url:   Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateProductRequest {
    pub category_id: i32,
    pub name:        String,
    pub description: Option<String>,
    pub price:       Option<i64>,
    pub unit:        Option<String>,
    pub image_url:   Option<String>,
}

// ── Products ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ProductBrowseQuery {
    pub category_id: Option<i32>,
    pub name:        Option<String>,
    pub min_price:   Option<i64>,
    pub max_price:   Option<i64>,
    /// Accepted values: "newest" (default) | "price_asc" | "price_desc"
    pub sort:        Option<String>,
    pub page:        Option<i64>,
}

// ── Agri-connect ──────────────────────────────────────────────────────────────

#[derive(Deserialize, Validate)]
pub struct SearchQuery {
    #[validate(range(min = LAT_MIN, max = LAT_MAX, message = "Latitude must be between -90 and 90"))]
    pub lat:        f64,
    #[validate(range(min = LNG_MIN, max = LNG_MAX, message = "Longitude must be between -180 and 180"))]
    pub lng:        f64,
    #[validate(range(min = RADIUS_MIN_M, max = RADIUS_MAX_M, message = "Radius must be between 100 and 50000 metres"))]
    pub radius:     Option<f64>,
    pub service_id: Option<i32>,
}

#[derive(Deserialize, Validate)]
pub struct BrowseQuery {
    pub service_id: Option<i32>,
    pub name:       Option<String>,
    /// Accepted values: "verified_first" (default) | "newest" | "name_asc"
    pub sort:       Option<String>,
    #[validate(range(min = 1, message = "Page number must be greater than 0"))]
    pub page:       Option<i64>,
}

#[derive(Deserialize)]
pub struct CategoriesQuery {
    #[serde(rename = "type")]
    pub cat_type: Option<String>,
}

#[derive(Deserialize, Validate)]
pub struct InquiryRequest {
    #[validate(range(min = 1, message = "Invalid provider"))]
    pub business_id:     i32,
    #[validate(length(min = 1, max = 255, message = "Topic must not be empty and at most 255 characters"))]
    pub topic:           String,
    #[validate(length(min = 1, max = 100, message = "Full name must not be empty and at most 100 characters"))]
    pub requester_name:  String,
    #[validate(custom(function = "validate_vn_phone"))]
    pub requester_phone: String,
    #[validate(length(max = 1000, message = "Note must be at most 1000 characters"))]
    pub note:            Option<String>,
    #[validate(range(min = 1, message = "Invalid service"))]
    pub service_id:      Option<i32>,
}
