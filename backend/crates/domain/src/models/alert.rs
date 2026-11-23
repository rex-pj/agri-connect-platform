use chrono::{DateTime, NaiveDate, Utc};

/// What farmers must know at once: a disease outbreak, salt water coming
/// up the rivers, a storm: one of the alert kinds the admin keeps
/// (`alert_kinds`, by code). It may concern one sector and one
/// province; none means every sector, the whole region.
#[derive(Debug, Clone)]
pub struct Alert {
    pub id: i32,
    pub kind: String,
    /// Its kind's name and icon.
    pub kind_name: String,
    pub kind_icon: Option<String>,
    pub sector: Option<String>,
    pub province_code: Option<String>,
    /// The province's name, when it has one.
    pub province_name: Option<String>,
    pub title: String,
    pub body: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub is_active: bool,
    /// Loaded with the demo data.
    pub is_demo: bool,
    pub updated_on: DateTime<Utc>,
}

/// An alert's boxes, validated (create and edit alike).
#[derive(Debug, Clone)]
pub struct AlertFields {
    pub kind: String,
    pub sector: Option<String>,
    pub province_code: Option<String>,
    pub title: String,
    pub body: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
}
