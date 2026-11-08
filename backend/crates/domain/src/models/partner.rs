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

/// Where a contract applies: a whole province, or one ward of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractArea {
    pub province_code: String,
    /// `None`: the whole province.
    pub ward_code: Option<String>,
}

impl ContractArea {
    /// Whether a place in this ward (of this province) is in the area.
    pub fn covers(&self, ward_code: &str, province_code: &str) -> bool {
        match &self.ward_code {
            Some(w) => w == ward_code,
            None => self.province_code == province_code,
        }
    }
}

/// What a partner pays for in one area: some categories, between two dates
/// (both included), a monthly fee in đồng, and the providers it sponsors.
pub struct Contract {
    pub id: i32,
    pub partner_id: i32,
    pub area: ContractArea,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub monthly_fee: i64,
    pub note: Option<String>,
    pub category_ids: Vec<i32>,
    pub sponsored: Vec<SponsoredBusiness>,
}

pub struct NewContract {
    pub partner_id: i32,
    pub area: ContractArea,
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
    pub area: ContractArea,
    pub monthly_fee: i64,
    pub category_ids: Vec<i32>,
    pub business_ids: Vec<i32>,
}
