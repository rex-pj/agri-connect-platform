pub struct ReviewAdmin {
    pub id:            i32,
    pub business_id:   i32,
    pub business_name: String,
    pub reviewer_name: String,
    pub rating:        i16,
    pub comment:       Option<String>,
    pub is_visible:    bool,
    pub created_on:    chrono::DateTime<chrono::Utc>,
}
