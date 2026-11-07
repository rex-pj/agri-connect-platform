use chrono::{DateTime, NaiveDate, Utc};

/// A company that pays for the demand report and sponsored places: a brand,
/// a dealer, a cooperative or an exporter (`PARTNER_KINDS`).
pub struct Partner {
    pub id: i32,
    pub name: String,
    pub kind: String,
    pub contact_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub is_active: bool,
    /// The partner account that signs in to read its report.
    pub account_username: Option<String>,
    pub created_on: DateTime<Utc>,
}

/// A partner's boxes, as stored (create and edit alike).
pub struct PartnerFields {
    pub name: String,
    pub kind: String,
    pub contact_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

/// A sponsored provider of a contract.
pub struct SponsoredBusiness {
    pub business_id: i32,
    pub name: String,
}

/// What a partner pays for in one area: some categories, between two dates
/// (both included), a monthly fee in đồng, and the providers it sponsors.
pub struct Contract {
    pub id: i32,
    pub partner_id: i32,
    pub location_id: i32,
    pub location_label: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub monthly_fee: i64,
    pub note: Option<String>,
    pub category_ids: Vec<i32>,
    pub sponsored: Vec<SponsoredBusiness>,
}

pub struct NewContract {
    pub partner_id: i32,
    pub location_id: i32,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub monthly_fee: i64,
    pub note: Option<String>,
    pub category_ids: Vec<i32>,
}

/// A contract in force on a day, of a partner that is on: what the revenue
/// figures, the partner's report and the sponsored places read.
pub struct ActiveContract {
    pub contract_id: i32,
    pub partner_id: i32,
    pub location_id: i32,
    pub monthly_fee: i64,
    pub category_ids: Vec<i32>,
    pub business_ids: Vec<i32>,
}
