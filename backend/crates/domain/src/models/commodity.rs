/// What a farmer raises ("cây trồng, vật nuôi"): rice, durian, pigs, shrimp. It is in
/// one sector (`sectors`); the price board groups its items by it
/// (`crops.commodity_id`), and a supply may say it is for one
/// (`products.commodity_id`: lúa giống ST25 is for rice).
pub struct Commodity {
    pub id: i32,
    pub name: String,
    pub sector: String,
    /// Font Awesome class.
    pub icon: Option<String>,
    pub display_order: i16,
    /// Days a price of its crops stays on the board (1 to
    /// `PRICE_FRESH_DAYS_MAX`).
    pub price_fresh_days: i16,
    pub is_active: bool,
    /// Demo data (`DemoDataRepositoryTrait`), not a real entry yet.
    pub is_demo: bool,
}

/// A commodity's boxes, as stored (create and edit alike).
pub struct CommodityFields {
    pub name: String,
    pub sector: String,
    pub icon: Option<String>,
    pub display_order: i16,
    /// Days a price of its crops stays on the board (1 to
    /// `PRICE_FRESH_DAYS_MAX`).
    pub price_fresh_days: i16,
}
