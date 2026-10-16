pub struct NewInquiry {
    pub business_id: i32,
    pub service_id: Option<i32>,
    pub topic: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub note: Option<String>,
}

pub struct InquiryAdmin {
    pub id: i32,
    pub business_id: i32,
    pub business_name: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub topic: String,
    pub status: String,
    pub created_on: chrono::DateTime<chrono::Utc>,
    pub updated_on: chrono::DateTime<chrono::Utc>,
    pub updated_by_name: Option<String>,
}

/// A contact request as the provider it was sent to sees it.
pub struct InquiryForOwner {
    pub id: i32,
    /// The service it was about, when the farmer chose one.
    pub service_name: Option<String>,
    pub topic: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub note: Option<String>,
    pub status: String,
    pub created_on: chrono::DateTime<chrono::Utc>,
}

/// A new inquiry and where to tell its provider about it.
pub struct InquiryForNotice {
    pub business_name: String,
    /// The account managing the business, else the business's own address;
    /// `None` when neither exists (nobody to tell).
    pub recipient_email: Option<String>,
    pub requester_name: String,
    pub requester_phone: String,
    pub topic: String,
    pub note: Option<String>,
}
