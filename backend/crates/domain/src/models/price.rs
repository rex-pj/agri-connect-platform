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

/// Words searched in the price list: a price matches when its crop or its
/// area is one of those whose name has them, or its note has them.
#[derive(Debug, Clone)]
pub struct PriceWords {
    pub text: String,
    pub crop_ids: Vec<i32>,
    pub location_ids: Vec<i32>,
}

/// What the admin's price list is narrowed to; every one set narrows it.
#[derive(Debug, Default, Clone)]
pub struct PriceFilter {
    pub crop_id: Option<i32>,
    /// The words searched, with the crops and areas whose name has them.
    pub words: Option<PriceWords>,
    pub location_id: Option<i32>,
    /// Observed on or after / on or before this day.
    pub observed_from: Option<NaiveDate>,
    pub observed_to: Option<NaiveDate>,
    /// Only prices older than their commodity's freshness window: asked by
    /// `stale`, worked out by the use case into `stale_groups` (crops with
    /// the same window, and the day before which a price is stale).
    pub stale: bool,
    pub stale_groups: Vec<(Vec<i32>, NaiveDate)>,
}
