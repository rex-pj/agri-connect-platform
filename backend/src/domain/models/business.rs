use chrono::{DateTime, Utc};

pub struct BusinessGeoResult {
    pub id:          i32,
    pub name:        String,
    pub phone:       String,
    pub address:     Option<String>,
    pub is_verified: bool,
    pub distance_m:  f64,
    pub services:    Vec<String>,
}

pub struct BusinessBrowse {
    pub id:          i32,
    pub name:        String,
    pub phone:       String,
    pub address:     Option<String>,
    pub is_verified: bool,
    pub services:    Vec<String>,
    pub created_on:  DateTime<Utc>,
}

pub struct BusinessProfile {
    pub id:          i32,
    pub name:        String,
    pub phone:       String,
    pub email:       Option<String>,
    pub description: Option<String>,
    pub address:     Option<String>,
    pub is_verified: bool,
    pub lat:         Option<f64>,
    pub lng:         Option<f64>,
}

pub struct BusinessAdminView {
    pub id:              i32,
    pub name:            String,
    pub phone:           String,
    pub address:         Option<String>,
    pub is_verified:     bool,
    pub is_active:       bool,
    pub service_count:   i64,
    pub product_count:   i64,
    pub created_on:      DateTime<Utc>,
    pub linked_username: Option<String>,
}

pub struct Service {
    pub id:          i32,
    pub name:        String,
    pub description: Option<String>,
    pub price_note:  Option<String>,
    pub category:    String,
}

pub struct BusinessDetail {
    pub id:          i32,
    pub name:        String,
    pub phone:       String,
    pub description: Option<String>,
    pub address:     Option<String>,
    pub is_verified: bool,
    pub services:    Vec<Service>,
}

pub struct ServiceAdminItem {
    pub id:          i32,
    pub name:        String,
    pub category:    String,
    pub price_note:  Option<String>,
    pub is_active:   bool,
}

pub struct ProductAdminItem {
    pub id:       i32,
    pub name:     String,
    pub price:    Option<i64>,
    pub unit:     Option<String>,
    pub category: String,
    pub is_active: bool,
}

pub struct BusinessAdminDetail {
    pub id:              i32,
    pub name:            String,
    pub phone:           String,
    pub email:           Option<String>,
    pub description:     Option<String>,
    pub address:         Option<String>,
    pub is_verified:     bool,
    pub is_active:       bool,
    pub created_on:      DateTime<Utc>,
    pub linked_username: Option<String>,
    pub services:        Vec<ServiceAdminItem>,
    pub products:        Vec<ProductAdminItem>,
}
