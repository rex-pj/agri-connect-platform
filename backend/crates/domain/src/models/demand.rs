use chrono::{DateTime, Utc};

/// How a farmer reached out: what the demand report counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeadChannel {
    Call,
    Zalo,
    Inquiry,
}

/// Which rows a report counts. Demo data never reaches a real partner's
/// report; a demo partner's counts only demo data; the admin's counts
/// everything there is, demo data while it is loaded (the page says so).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DemoScope {
    #[default]
    Real,
    Demo,
    All,
}

impl DemoScope {
    /// The one `is_demo` value counted, or `None` for both.
    pub fn only(self) -> Option<bool> {
        match self {
            Self::Real => Some(false),
            Self::Demo => Some(true),
            Self::All => None,
        }
    }
}

/// One lead of a business in the report's `DemoScope`: a call or Zalo tap, or a contact
/// request that is not spam. Never who sent it, except a request's phone,
/// which only tells two requests from the same person apart.
#[derive(Debug, Clone)]
pub struct LeadEvent {
    pub at: DateTime<Utc>,
    pub business_id: i32,
    /// The category of the service or product it was about; `None` for a tap
    /// on a shop's own page or a request with no service chosen.
    pub category_id: Option<i32>,
    pub channel: LeadChannel,
    /// The requester's phone, for an inquiry only.
    pub requester_phone: Option<String>,
}

/// The ward a business counts under: the one its position lies in (or the
/// nearest within `ADMIN_UNIT_SNAP_M`). Businesses without one are absent.
pub struct BusinessArea {
    pub business_id: i32,
    pub ward_code: String,
    pub name: String,
    pub phone: String,
    pub address: Option<String>,
    pub is_demo: bool,
}

/// A business with exactly one live service, and that service's category:
/// a tap on its shop page is about that service.
pub struct SoleCategory {
    pub business_id: i32,
    pub category_id: i32,
}

/// A search to store: the words (normalised), the category filter, the ward
/// (of the preset chosen, or of the searcher's position) and how many
/// results came back. Never who searched.
pub struct NewSearch {
    pub query: Option<String>,
    pub category_id: Option<i32>,
    pub ward_code: Option<String>,
    pub results: i64,
}

/// A stored search, as the demand report reads it.
pub struct SearchEvent {
    pub at: DateTime<Utc>,
    pub query: Option<String>,
    pub category_id: Option<i32>,
    pub ward_code: Option<String>,
    pub results: i64,
}
