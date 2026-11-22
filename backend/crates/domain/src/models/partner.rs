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
    /// Demo data: its report counts only demo data.
    pub is_demo: bool,
    /// The partner account that signs in to read its report.
    pub account_id: Option<i32>,
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

/// A shop lying in a contract's area, as the sponsor search lists it.
pub struct AreaShop {
    pub business_id: i32,
    pub name: String,
    pub phone: String,
    pub address: Option<String>,
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

/// A contract in force on a day, of a partner that is on, as the revenue
/// figures read it: whose, where, and its monthly fee.
pub struct ContractFee {
    pub contract_id: i32,
    pub partner_id: i32,
    pub area: ContractArea,
    pub monthly_fee: i64,
}

/// A contract in force with what it covers: its categories and sponsored
/// shops (the partner's report, the sponsored places).
pub struct ActiveContract {
    pub contract_id: i32,
    pub partner_id: i32,
    pub area: ContractArea,
    pub monthly_fee: i64,
    pub category_ids: Vec<i32>,
    pub business_ids: Vec<i32>,
}

/// What a partner's contract list is narrowed to; every one set narrows it.
#[derive(Debug, Default, Clone)]
pub struct ContractFilter {
    /// One of `CONTRACT_LIST_STATES`.
    pub state: Option<&'static str>,
    /// One area, as its text (`p82`, `w28439`).
    pub area: Option<String>,
    /// The monthly fee between these, in đồng.
    pub fee_min: Option<i64>,
    pub fee_max: Option<i64>,
    /// Begins between these days, ends between these days.
    pub start_from: Option<NaiveDate>,
    pub start_to: Option<NaiveDate>,
    pub end_from: Option<NaiveDate>,
    pub end_to: Option<NaiveDate>,
    /// Covers this category.
    pub category_id: Option<i32>,
}

impl ContractFilter {
    pub fn is_empty(&self) -> bool {
        self.state.is_none()
            && self.area.is_none()
            && self.fee_min.is_none()
            && self.fee_max.is_none()
            && self.start_from.is_none()
            && self.start_to.is_none()
            && self.end_from.is_none()
            && self.end_to.is_none()
            && self.category_id.is_none()
    }
}
