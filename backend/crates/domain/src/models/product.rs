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
    pub business_id: i32,
    pub business_name: String,
    pub image_file_id: Option<Uuid>,
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
}

pub struct ProductView {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub category: String,
    /// Font Awesome class of the category, for the no-image placeholder.
    pub category_icon: Option<String>,
    pub vendor_name: String,
    pub vendor_phone: String,
    pub image_file_id: Option<Uuid>,
    pub is_available: bool,
    pub created_on: DateTime<Utc>,
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
    pub is_active: bool,
    pub created_on: DateTime<Utc>,
}

pub struct NewProduct {
    pub business_id: i32,
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
    pub created_by: i32,
}

pub struct UpdateProduct {
    pub id: i32,
    pub owner_user: i32,
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
}

/// Public product search criteria; every field narrows the result.
#[derive(Debug, Default)]
pub struct ProductFilter {
    pub category_id: Option<i32>,
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
