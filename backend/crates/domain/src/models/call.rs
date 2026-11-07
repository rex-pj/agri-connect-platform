/// A tap on a public call button, resolved to the business it rings. Only what
/// was called, never who called.
pub struct NewCallClick {
    pub business_id: i32,
    /// The service or product whose card or page the button was on.
    pub service_id: Option<i32>,
    pub product_id: Option<i32>,
    /// One of `CALL_CHANNELS`.
    pub channel: String,
}
