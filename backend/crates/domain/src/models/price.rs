use chrono::NaiveDate;

/// Farm produce whose farm-gate price is followed ("nông sản"): a variety or
/// grade of one commodity ("Lúa ST25", "Heo hơi"), priced per `unit` ("kg").
pub struct Crop {
    pub id: i32,
    pub name: String,
    /// The commodity it is of (the board groups by it).
    pub commodity_id: i32,
    pub unit: String,
    pub display_order: i32,
    pub is_active: bool,
    /// Demo data (`DemoDataRepositoryTrait`), not a real entry yet.
    pub is_demo: bool,
}

/// A crop's boxes, as stored (create and edit alike).
pub struct CropFields {
    pub name: String,
    pub commodity_id: i32,
    pub unit: String,
    pub display_order: i32,
}

/// The price of a crop in one area on one day: a range in đồng per unit.
pub struct CropPrice {
    pub id: i32,
    pub crop_id: i32,
    pub crop_name: String,
    pub unit: String,
    pub location_id: i32,
    pub location_label: String,
    pub observed_on: NaiveDate,
    pub price_low: i64,
    pub price_high: i64,
    pub note: Option<String>,
}

/// A price as entered; the same crop, area and day again replaces it.
pub struct NewCropPrice {
    pub crop_id: i32,
    pub location_id: i32,
    pub observed_on: NaiveDate,
    pub price_low: i64,
    pub price_high: i64,
    pub note: Option<String>,
}
