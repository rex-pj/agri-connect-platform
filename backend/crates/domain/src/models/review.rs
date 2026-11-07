use chrono::{DateTime, Utc};

pub struct ReviewAdmin {
    pub id: i32,
    pub business_id: i32,
    pub business_name: String,
    pub reviewer_name: String,
    pub rating: i16,
    pub comment: Option<String>,
    /// The provider's public answer, if any.
    pub reply: Option<String>,
    pub is_visible: bool,
    pub created_on: DateTime<Utc>,
    pub updated_on: DateTime<Utc>,
    pub updated_by_name: Option<String>,
}

/// A farmer's review of a provider they reached through the platform:
/// `inquiry_id` is that contact request. Stored hidden until an admin shows it.
pub struct NewReview {
    pub business_id: i32,
    pub inquiry_id: i32,
    pub rating: i16,
    pub comment: Option<String>,
    pub reviewer_name: String,
    /// SHA-256 of the phone number: one review per phone and provider.
    pub reviewer_phone_hash: String,
}

/// A shown review as farmers see it on the provider's page.
pub struct PublicReview {
    pub id: i32,
    pub reviewer_name: String,
    pub rating: i16,
    pub comment: Option<String>,
    pub reply: Option<String>,
    pub created_on: DateTime<Utc>,
}

/// The shown reviews of a provider, in figures.
pub struct RatingSummary {
    pub count: i64,
    /// `None` without any shown review.
    pub average: Option<f64>,
}

/// A review of the provider's own business, shown or still waiting for an
/// admin, as their dashboard lists it.
pub struct OwnReview {
    pub id: i32,
    pub reviewer_name: String,
    pub rating: i16,
    pub comment: Option<String>,
    pub reply: Option<String>,
    pub is_visible: bool,
    pub created_on: DateTime<Utc>,
}
