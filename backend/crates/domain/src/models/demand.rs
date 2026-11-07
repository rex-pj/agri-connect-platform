use chrono::{DateTime, Utc};

/// How a farmer reached out: what the demand report counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeadChannel {
    Call,
    Zalo,
    Inquiry,
}

/// One lead of a real (not demo) business: a call or Zalo tap, or a contact
/// request that is not spam. Never who sent it, except a request's phone,
/// which only tells two requests from the same person apart.
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

/// The location preset a business counts under: the nearest active one
/// within the report radius. Businesses without a position are absent.
pub struct BusinessArea {
    pub business_id: i32,
    pub location_id: i32,
}

/// A business with exactly one live service, and that service's category:
/// a tap on its shop page is about that service.
pub struct SoleCategory {
    pub business_id: i32,
    pub category_id: i32,
}

/// A search to store: the words (normalised), the category filter, the area
/// (a preset, or the nearest one to the searcher's position) and how many
/// results came back. Never who searched.
pub struct NewSearch {
    pub query: Option<String>,
    pub category_id: Option<i32>,
    pub location_id: Option<i32>,
    pub results: i64,
}

/// A stored search, as the demand report reads it.
pub struct SearchEvent {
    pub at: DateTime<Utc>,
    pub query: Option<String>,
    pub category_id: Option<i32>,
    pub location_id: Option<i32>,
    pub results: i64,
}
