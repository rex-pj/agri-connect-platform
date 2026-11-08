//! HTTP request shapes. Field rules live in the use cases, not here, so the
//! JSON API and the server-rendered forms enforce exactly the same ones.

use application::inputs::{ReviewInput, ServiceInput, UpdateProfileInput};
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
    /// Narrows a searchable list to names containing it.
    pub name: Option<String>,
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

/// `text`: the category form's name or slug as typed; `id`: the category being
/// edited, absent on create.
#[derive(Deserialize)]
pub struct SlugSuggestionQuery {
    #[serde(default)]
    pub text: String,
    pub id: Option<i32>,
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

/// POST /api/admin/locations and PUT /api/admin/locations/{id}.
#[derive(Deserialize)]
pub struct LocationRequest {
    #[serde(default)]
    pub label: String,
    /// Left out with a `ward`: the ward's point.
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub display_order: i32,
    /// `w28471`: fills a blank label and a missing point from that ward.
    pub ward: Option<String>,
}

impl From<LocationRequest> for application::inputs::LocationInput {
    fn from(r: LocationRequest) -> Self {
        Self {
            label: r.label,
            lat: r.lat.unwrap_or(f64::NAN),
            lng: r.lng.unwrap_or(f64::NAN),
            display_order: r.display_order,
            ward: r.ward,
        }
    }
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

impl From<UpdateProfileRequest> for UpdateProfileInput {
    fn from(r: UpdateProfileRequest) -> Self {
        Self {
            name: r.name,
            phone: r.phone,
            description: r.description,
            address: r.address,
            lat: r.lat,
            lng: r.lng,
        }
    }
}

/// GET /api/provider/reviews is a list; PUT /api/provider/reviews/{id}/reply
/// sets the answer (blank or absent: no answer).
#[derive(Deserialize)]
pub struct ReviewReplyRequest {
    #[serde(default)]
    pub reply: Option<String>,
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
    /// Names containing it (the record's or its business's).
    pub name: Option<String>,
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

#[derive(Deserialize)]
pub struct AdminUpdateProductRequest {
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
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

/// POST /api/explore/reviews: a farmer's review of a provider they reached
/// through the platform, from the phone their contact request came from.
#[derive(Deserialize)]
pub struct ReviewRequest {
    pub business_id: i32,
    pub reviewer_name: String,
    pub reviewer_phone: String,
    pub rating: Option<i16>,
    pub comment: Option<String>,
}

impl From<ReviewRequest> for ReviewInput {
    fn from(r: ReviewRequest) -> Self {
        Self {
            business_id: r.business_id,
            reviewer_name: r.reviewer_name,
            reviewer_phone: r.reviewer_phone,
            rating: r.rating,
            comment: r.comment,
        }
    }
}

/// POST /api/explore/calls: a tap on a public call button (sent by the
/// `callBeacon` Alpine component).
#[derive(Deserialize)]
pub struct CallRequest {
    pub kind: String,
    pub id: i32,
    /// `call` (the default) or `zalo`.
    #[serde(default)]
    pub channel: Option<String>,
}

/// `?area=` of the demand report: a province (`p82`), a ward (`w28439`),
/// `none` (businesses with no ward), or empty for every area.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct DemandQuery {
    pub area: String,
}

impl DemandQuery {
    /// The area in focus; anything unknown is all of them.
    pub fn focus(&self) -> Option<application::admin_units::AreaKey> {
        application::admin_units::AreaKey::parse(&self.area)
    }
}

/// POST /api/admin/partners and PUT /api/admin/partners/{id}.
#[derive(Deserialize)]
pub struct PartnerRequest {
    pub name: String,
    pub kind: String,
    pub contact_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

impl From<PartnerRequest> for application::inputs::PartnerInput {
    fn from(r: PartnerRequest) -> Self {
        Self {
            name: r.name,
            kind: r.kind,
            contact_name: r.contact_name,
            phone: r.phone,
            email: r.email,
        }
    }
}

/// POST /api/admin/partners/{id}/contracts. Dates are `YYYY-MM-DD`; a date
/// that does not parse is reported by the use case like a missing one.
#[derive(Deserialize)]
pub struct ContractRequest {
    /// `p82` (a whole province) or `w28439` (one ward); ignored on an edit.
    pub area: Option<String>,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
    pub monthly_fee: Option<i64>,
    pub note: Option<String>,
    #[serde(default)]
    pub category_ids: Vec<i32>,
}

impl From<ContractRequest> for application::inputs::ContractInput {
    fn from(r: ContractRequest) -> Self {
        let date = |s: Option<String>| {
            s.and_then(|s| chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok())
        };
        Self {
            area: r.area,
            starts_on: date(r.starts_on),
            ends_on: date(r.ends_on),
            monthly_fee: r.monthly_fee,
            note: r.note,
            category_ids: r.category_ids,
        }
    }
}

/// POST /api/admin/partners/{id}/contracts/{cid}/sponsored.
#[derive(Deserialize)]
pub struct SponsorRequest {
    pub business_id: i32,
}

/// GET /api/explore/prices: one area, or every area.
#[derive(Deserialize)]
pub struct PriceBoardQuery {
    pub location_id: Option<i32>,
}

/// POST /api/admin/crops, PUT /api/admin/crops/{id}.
#[derive(Deserialize)]
pub struct CropRequest {
    pub name: String,
    pub kind: String,
    pub unit: String,
    #[serde(default)]
    pub display_order: i32,
}

impl From<CropRequest> for application::inputs::CropInput {
    fn from(r: CropRequest) -> Self {
        Self {
            name: r.name,
            kind: r.kind,
            unit: r.unit,
            display_order: r.display_order,
        }
    }
}

/// POST /api/admin/prices. The day is `YYYY-MM-DD`; one that does not parse
/// is refused like a missing one. No `price_high`: one price (high = low).
#[derive(Deserialize)]
pub struct PriceRequest {
    pub crop_id: Option<i32>,
    pub location_id: Option<i32>,
    pub observed_on: Option<String>,
    pub price_low: Option<i64>,
    pub price_high: Option<i64>,
    pub note: Option<String>,
}

impl From<PriceRequest> for application::inputs::PriceInput {
    fn from(r: PriceRequest) -> Self {
        Self {
            crop_id: r.crop_id,
            location_id: r.location_id,
            observed_on: r
                .observed_on
                .and_then(|s| chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()),
            price_low: r.price_low,
            price_high: r.price_high.or(r.price_low),
            note: r.note,
        }
    }
}

/// POST /api/explore/jobs. Where: `lat`/`lng` (the phone's position) or a
/// `location_id` preset.
#[derive(Deserialize)]
pub struct JobRequest {
    pub category_id: Option<i32>,
    pub location_id: Option<i32>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub requester_name: String,
    #[serde(default)]
    pub requester_phone: String,
}

impl From<JobRequest> for application::inputs::JobInput {
    fn from(r: JobRequest) -> Self {
        Self {
            category_id: r.category_id,
            location_id: r.location_id,
            lat: r.lat,
            lng: r.lng,
            description: r.description,
            requester_name: r.requester_name,
            requester_phone: r.requester_phone,
        }
    }
}

/// PATCH /api/admin/jobs/{id}/status.
#[derive(Deserialize)]
pub struct JobStatusRequest {
    pub status: String,
    pub reason: Option<String>,
}

/// POST /api/zalo/location and /api/zalo/phone: the user's Zalo access token
/// (`getAccessToken`) and the Mini App's one-time token.
#[derive(Deserialize)]
pub struct ZaloTokenRequest {
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub token: String,
}
