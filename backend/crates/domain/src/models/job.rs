use chrono::{DateTime, Utc};

/// A job as posted, at a point (`SRID=4326;POINT(lng lat)`).
pub struct NewJobPost {
    pub category_id: i32,
    pub location_id: Option<i32>,
    pub point_wkt: String,
    pub description: String,
    pub requester_name: String,
    pub requester_phone: String,
}

/// A job post as the admin and a provider see it. `distance_m` is from the
/// provider's business (none in the admin's list).
pub struct JobPost {
    pub id: i32,
    pub category_id: i32,
    pub category: String,
    pub area: Option<String>,
    pub description: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub status: String,
    pub created_on: DateTime<Utc>,
    pub distance_m: Option<f64>,
}

/// Who to tell about a new job: a nearby provider and its address.
pub struct JobRecipient {
    pub business_name: String,
    pub email: String,
}

/// A new job and the providers to tell, nearest first.
pub struct JobForNotice {
    pub category: String,
    pub area: Option<String>,
    pub description: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub recipients: Vec<JobRecipient>,
}
