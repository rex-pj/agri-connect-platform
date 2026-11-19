use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct BusinessGeoResult {
    pub id: i32,
    pub name: String,
    pub phone: String,
    pub address: Option<String>,
    pub is_verified: bool,
    pub distance_m: f64,
    pub services: Vec<String>,
}

pub struct BusinessBrowse {
    pub id: i32,
    pub name: String,
    pub phone: String,
    pub address: Option<String>,
    pub is_verified: bool,
    pub services: Vec<String>,
    pub created_on: DateTime<Utc>,
}

pub struct BusinessProfile {
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

/// A provider's own profile as they save it, already validated.
pub struct ProfileUpdate {
    pub name: String,
    pub phone: String,
    pub description: Option<String>,
    pub address: Option<String>,
    /// `SRID=4326;POINT(lng lat)`, or `None` for no location.
    pub location_wkt: Option<String>,
}

pub struct BusinessAdminView {
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

pub struct Service {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
    pub category: String,
    /// Font Awesome class of the category.
    pub category_icon: Option<String>,
    /// Its photo, if the provider added one.
    pub image_file_id: Option<uuid::Uuid>,
}

pub struct BusinessDetail {
    pub id: i32,
    pub name: String,
    pub phone: String,
    pub description: Option<String>,
    pub address: Option<String>,
    pub is_verified: bool,
    pub services: Vec<Service>,
}

pub struct ServiceAdminItem {
    pub id: i32,
    pub name: String,
    pub category: String,
    pub price_note: Option<String>,
    pub is_active: bool,
    pub image_file_id: Option<Uuid>,
}

pub struct ProductAdminItem {
    pub id: i32,
    pub name: String,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub category: String,
    pub is_active: bool,
    pub image_file_id: Option<Uuid>,
}

pub struct BusinessAdminDetail {
    pub id: i32,
    pub name: String,
    pub phone: String,
    pub email: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub is_verified: bool,
    pub is_active: bool,
    pub is_demo: bool,
    /// The ward its position lies in, with the names of that ward
    /// ("Xã Bình Phú") and its province; `None` without a position.
    pub ward_code: Option<String>,
    pub ward_name: Option<String>,
    pub province_name: Option<String>,
    pub created_on: DateTime<Utc>,
    /// The managing account, for the link to it.
    pub linked_user_id: Option<i32>,
    pub linked_username: Option<String>,
    /// Whether that account confirmed its email address: notices (contact
    /// requests, job posts) reach it only then.
    pub email_verified: bool,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub services: Vec<ServiceAdminItem>,
    pub products: Vec<ProductAdminItem>,
}

/// Order of a provider listing. The wire names are query-string values that
/// shared links carry, so renaming one breaks those links.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProviderSort {
    /// Verified providers first, then the newest.
    #[default]
    VerifiedFirst,
    Newest,
    NameAsc,
}

impl ProviderSort {
    pub const ALL: [ProviderSort; 3] = [Self::VerifiedFirst, Self::Newest, Self::NameAsc];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VerifiedFirst => "verified_first",
            Self::Newest => "newest",
            Self::NameAsc => "name_asc",
        }
    }

    /// Unknown or missing means the default order, never an error.
    pub fn parse(value: Option<&str>) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| Some(s.as_str()) == value)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderSort;

    #[test]
    fn sort_names_round_trip_and_unknown_means_default() {
        for sort in ProviderSort::ALL {
            assert_eq!(ProviderSort::parse(Some(sort.as_str())), sort);
        }
        assert_eq!(
            ProviderSort::parse(Some("rating")),
            ProviderSort::VerifiedFirst
        );
    }
}

/// Who changes a listing (a service, a supply) of a business: its owner,
/// matched in the same statement against the businesses they manage, or a
/// moderator, any live listing. Either is stamped as its last editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingEditor {
    Owner(i32),
    Moderator(i32),
}

impl ListingEditor {
    pub fn user_id(self) -> i32 {
        match self {
            Self::Owner(id) | Self::Moderator(id) => id,
        }
    }
}

/// One listing of a business, for the figures about it alone (taps on its
/// call button, contact requests about it). Its business goes with it, so a
/// count reads that business's rows by their index first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingRef {
    Service { business_id: i32, id: i32 },
    Product { business_id: i32, id: i32 },
}
