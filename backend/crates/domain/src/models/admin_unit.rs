//! Vietnam's administrative units since 1 July 2025: 34 provinces (some are
//! centrally-run cities) and their wards, communes and special zones. No
//! districts. Codes are the official ones (Tổng cục Thống kê).

/// A province or centrally-run city. The point is inside it.
#[derive(Debug, Clone)]
pub struct Province {
    pub code: String,
    /// "Đồng Tháp".
    pub name: String,
    /// "Tỉnh Đồng Tháp".
    pub full_name: String,
    /// `province` or `city`.
    pub unit_type: String,
    pub lat: f64,
    pub lng: f64,
    /// False once a dataset no longer lists it; kept for what refers to it.
    pub is_active: bool,
}

/// A ward, commune or special zone. The point is inside it; its boundary
/// stays in the database, where the point-in-area queries run.
#[derive(Debug, Clone)]
pub struct Ward {
    pub code: String,
    pub province_code: String,
    /// "Cai Lậy".
    pub name: String,
    /// "Phường Cai Lậy".
    pub full_name: String,
    /// `ward`, `commune` or `special_zone`.
    pub unit_type: String,
    pub lat: f64,
    pub lng: f64,
    pub is_active: bool,
}

/// What loading the bundled dataset did: nothing when that version was
/// already in the database.
#[derive(Debug, Clone, Default)]
pub struct AdminUnitSync {
    pub version: String,
    pub loaded: bool,
    pub provinces: u64,
    pub wards: u64,
}
