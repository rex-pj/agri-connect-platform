use chrono::{DateTime, Utc};

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
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
}

pub struct NewService {
    pub business_id: i32,
    pub category_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
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
