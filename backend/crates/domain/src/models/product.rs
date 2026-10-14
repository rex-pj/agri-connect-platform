use chrono::{DateTime, Utc};

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
    pub vendor_name: String,
    pub vendor_phone: String,
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
