use chrono::NaiveDate;

/// A crop whose farm-gate price is followed: a rice variety or a fruit
/// (`CROP_KINDS`), priced per `unit` ("kg").
pub struct Crop {
    pub id: i32,
    pub name: String,
    pub kind: String,
    pub unit: String,
    pub display_order: i32,
    pub is_active: bool,
}

/// A crop's boxes, as stored (create and edit alike).
pub struct CropFields {
    pub name: String,
    pub kind: String,
    pub unit: String,
    pub display_order: i32,
}

/// The price of a crop in one area on one day: a range in đồng per unit.
pub struct CropPrice {
    pub id: i32,
    pub crop_id: i32,
    pub crop_name: String,
    pub crop_kind: String,
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
