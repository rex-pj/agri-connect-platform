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
