use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct AdminService {
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
    /// The account that added it and the business's managing account: equal
    /// when the provider listed it themselves rather than an admin for them.
    pub created_by: Option<i32>,
    pub owner_id: Option<i32>,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
}

/// One of a provider's own services, as their dashboard shows it: hidden ones
/// too, since an admin may have hidden it.
pub struct ProviderService {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
    pub category: String,
    pub category_id: i32,
    /// Font Awesome class of the category.
    pub category_icon: Option<String>,
    pub is_active: bool,
    pub image_file_id: Option<Uuid>,
    pub created_on: DateTime<Utc>,
}

pub struct NewService {
    pub business_id: i32,
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
    /// Its photo, stored already; inserted with the row.
    pub image_file_id: Option<Uuid>,
    pub created_by: i32,
}

pub struct UpdateService {
    pub id: i32,
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
    pub updated_by: i32,
}

/// A provider's edit of one of their own services. Visibility is not here:
/// `is_active` is the admin's moderation switch, never the owner's.
pub struct UpdateOwnService {
    pub id: i32,
    pub owner_user: i32,
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
}

/// A service as farmers see it, in a list or on its own page: live (shown,
/// not deleted, in a shown category, of a live business), with the business
/// that offers it.
pub struct PublicService {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
    pub category_id: i32,
    pub category: String,
    /// For the link to the category's page.
    pub category_slug: String,
    /// Font Awesome class of the category.
    pub category_icon: Option<String>,
    pub image_file_id: Option<Uuid>,
    pub business_id: i32,
    pub business_name: String,
    pub business_phone: String,
    pub business_address: Option<String>,
    pub business_verified: bool,
    pub created_on: DateTime<Utc>,
    /// Metres from the searched point; only on a near-me search.
    pub distance_m: Option<f64>,
}

/// What a service listing is narrowed to.
#[derive(Debug, Clone, Default)]
pub struct PublicServiceFilter {
    pub category_id: Option<i32>,
    /// Matched against the service's name and description and the
    /// business's name, ignoring case and diacritics.
    pub text: Option<String>,
}
