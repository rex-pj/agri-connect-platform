use crate::models::admin_list::AdminSort;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct AdminProduct {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub category: String,
    pub category_id: i32,
    /// The commodity it is for; `None`: every one.
    pub commodity_id: Option<i32>,
    pub commodity: Option<String>,
    /// Its sector: its commodity's, else its type's; `None`: every sector.
    pub sector: Option<String>,
    pub business_id: i32,
    pub business_name: String,
    pub image_file_id: Option<Uuid>,
    pub is_active: bool,
    /// Its business is on, its supply type shown: with `is_active`, what
    /// puts it on the public pages (`is_public`).
    pub business_active: bool,
    pub category_active: bool,
    /// Its provider's phone and badge, said at the top of its admin page.
    pub business_phone: String,
    pub business_verified: bool,
    /// The account that added it and the business's managing account: equal
    /// when the provider listed it themselves rather than an admin for them.
    pub created_by: Option<i32>,
    pub owner_id: Option<i32>,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
}

impl AdminProduct {
    /// On the public pages: shown, its business on, its type shown (the
    /// product repository's public query, read back for one row).
    pub fn is_public(&self) -> bool {
        self.is_active && self.business_active && self.category_active
    }
}

pub struct ProductView {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub category: String,
    /// The category's id and slug, for the link to its list.
    pub category_id: i32,
    pub category_slug: String,
    /// Font Awesome class of the category, for the no-image placeholder.
    pub category_icon: Option<String>,
    /// The commodity it is for ("Lúa"), and its id; `None`: every one.
    pub commodity_id: Option<i32>,
    pub commodity: Option<String>,
    pub vendor_name: String,
    pub vendor_phone: String,
    /// The seller carries the verified badge.
    pub vendor_verified: bool,
    pub image_file_id: Option<Uuid>,
    /// The provider selling it, for the link to its page.
    pub business_id: i32,
    pub created_on: DateTime<Utc>,
    /// Metres from the searched point; only on a near-me search.
    pub distance_m: Option<f64>,
}

pub struct ProviderProduct {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub category: String,
    pub image_file_id: Option<Uuid>,
    pub category_id: i32,
    pub commodity_id: Option<i32>,
    pub commodity: Option<String>,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
}

pub struct NewProduct {
    pub business_id: i32,
    pub category_id: i32,
    pub commodity_id: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
    /// Its photo, stored already; inserted with the row.
    pub image_file_id: Option<Uuid>,
    pub created_by: i32,
}

pub struct UpdateProduct {
    pub id: i32,
    pub owner_user: i32,
    pub category_id: i32,
    pub commodity_id: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
}

/// An admin's edit of any product (no owner check; visibility is its own
/// switch).
pub struct UpdateProductAdmin {
    pub id: i32,
    pub category_id: i32,
    pub commodity_id: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub updated_by: i32,
}

/// The admin's supply list criteria; every one set narrows it.
#[derive(Debug, Default, Clone)]
pub struct AdminProductFilter {
    pub provider_id: Option<i32>,
    /// One supply type's.
    pub category_id: Option<i32>,
    /// Those for one commodity.
    pub commodity_id: Option<i32>,
    pub search: Option<String>,
    /// `shown` / `hidden` (`admin_list::LISTING_LIST_STATES`).
    pub state: Option<&'static str>,
    /// `None`: the newest first.
    pub sort: Option<AdminSort>,
    /// Those of one sector (`supply_in_sector`).
    pub sector: Option<String>,
    /// What a supply lacks: `no_image` or `no_price`.
    pub gap: Option<String>,
    /// The price between these, in đồng.
    pub price_min: Option<i64>,
    pub price_max: Option<i64>,
    /// Made on or after / on or before this day (Vietnam time).
    pub created_from: Option<chrono::NaiveDate>,
    pub created_to: Option<chrono::NaiveDate>,
}

/// Public product search criteria; every field narrows the result.
#[derive(Debug, Default)]
pub struct ProductFilter {
    pub category_id: Option<i32>,
    /// Only supplies for this commodity, and those for every one.
    pub commodity_id: Option<i32>,
    /// Only supplies of this sector: for a commodity of it, or for none and of
    /// a type of it or of every sector.
    pub sector: Option<String>,
    /// One seller's products (their public page).
    pub business_id: Option<i32>,
    pub name: Option<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
}

/// Order of a product listing. The wire names are query-string values that
/// shared links carry, so renaming one breaks those links.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProductSort {
    #[default]
    Newest,
    PriceAsc,
    PriceDesc,
}

impl ProductSort {
    pub const ALL: [ProductSort; 3] = [Self::Newest, Self::PriceAsc, Self::PriceDesc];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Newest => "newest",
            Self::PriceAsc => "price_asc",
            Self::PriceDesc => "price_desc",
        }
    }

    /// Unknown or missing means newest: an old or hand-edited link should
    /// still show products, not an error.
    pub fn parse(value: Option<&str>) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| Some(s.as_str()) == value)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::ProductSort;

    #[test]
    fn sort_names_round_trip_and_unknown_means_newest() {
        for sort in ProductSort::ALL {
            assert_eq!(ProductSort::parse(Some(sort.as_str())), sort);
        }
        assert_eq!(ProductSort::parse(Some("cheapest")), ProductSort::Newest);
        assert_eq!(ProductSort::parse(None), ProductSort::Newest);
    }
}
