// Input parameter types passed from handlers into use cases.
// These are internal application-layer types — not HTTP request structs.

use domain::models::business::ProviderSort;

// ── Agri-connect ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct GeoSearchParams {
    pub lat: f64,
    pub lng: f64,
    pub radius: f64,
    pub service_id: Option<i32>,
}

pub struct BrowseParams {
    pub service_id: Option<i32>,
    /// The visitor's sector: its categories and those of every sector.
    pub sector: Option<String>,
    pub name: Option<String>,
    pub sort: ProviderSort,
    pub page: i64,
}

pub struct ContactInput {
    pub business_id: i32,
    pub topic: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub note: Option<String>,
    pub service_id: Option<i32>,
    /// The supply whose page the form was on.
    pub product_id: Option<i32>,
    /// Who sent it (`client_ip::client_key`; empty: unknown), hashed before
    /// it is kept in memory, never stored.
    pub client: String,
}

/// A farmer's review of a provider, from the phone their contact request
/// came from.
pub struct ReviewInput {
    pub business_id: i32,
    pub reviewer_name: String,
    pub reviewer_phone: String,
    /// 1 to 5 stars; `None` when the box was left empty or was not a number.
    pub rating: Option<i16>,
    pub comment: Option<String>,
}

// ── Provider dashboard ────────────────────────────────────────────────────────

pub struct UpdateProfileInput {
    pub name: String,
    pub phone: String,
    pub description: Option<String>,
    pub address: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    /// The ward chosen from the list (`w28439`): without a point, or with
    /// one in another ward, the business is placed at its centre
    /// (`PlaceUseCase::placed`).
    pub ward: Option<String>,
}

pub struct CreateProductInput {
    pub category_id: i32,
    /// The commodity it is for; `None`: every one.
    pub commodity_id: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
}

pub struct UpdateProductInput {
    pub product_id: i32,
    pub category_id: i32,
    pub commodity_id: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub unit: Option<String>,
}

/// A provider's own service, for create and update alike.
pub struct ServiceInput {
    pub category_id: i32,
    /// Blank takes the category's name.
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
}

// ── Admin catalogue ───────────────────────────────────────────────────────────

/// Editable category fields, shared by create and update.
pub struct CategoryInput {
    pub name: String,
    pub slug: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub display_order: i16,
    /// A sector code, or empty for every sector.
    pub sector: String,
}

/// A sector's or an alert kind's boxes (admin create and edit; the code
/// is made from the name once).
pub struct CodedInput {
    pub name: String,
    pub icon: Option<String>,
    pub display_order: i16,
}

/// A commodity's boxes (admin create and edit).
pub struct CommodityInput {
    pub name: String,
    /// A sector code (`sectors.code`).
    pub sector: String,
    pub icon: Option<String>,
    pub display_order: i16,
    /// Days a price of its crops stays on the board (1 to
    /// `PRICE_FRESH_DAYS_MAX`).
    pub price_fresh_days: i16,
}

/// Who made a tap or a search, as far as counting it goes. Every account is
/// an admin, a provider or a partner (farmers never sign in), so a
/// signed-in visitor's taps and searches are never demand. `key` tells one
/// anonymous visitor's repeats apart; it is kept in memory only, never
/// stored.
pub enum Visitor {
    SignedIn,
    Anonymous { key: String },
}
/// A tap on a public call button: which kind of card or page (`service`,
/// `product`, `provider`) and that record's id. The business it rings is
/// looked up from the record, never taken from the request. `channel` is how
/// the farmer reached out (`call`, `zalo`); none is a call.
pub struct CallInput {
    pub kind: String,
    pub id: i32,
    pub channel: Option<String>,
    pub visitor: Visitor,
}

/// Where a search looked: a location preset, a point (the device), or
/// nowhere in particular.
pub enum SearchPlace {
    Preset(i32),
    Point { lat: f64, lng: f64 },
    Anywhere,
}

/// A search a visitor made, to count in the demand report.
pub struct SearchInput {
    pub query: String,
    pub category_id: Option<i32>,
    pub place: SearchPlace,
    pub results: i64,
    pub visitor: Visitor,
}

/// A location preset's boxes (create and edit alike). With a ward (`w28471`), a blank label takes the
/// ward's preset name and a missing coordinate (not finite) its point.
pub struct LocationInput {
    pub label: String,
    pub lat: f64,
    pub lng: f64,
    pub display_order: i32,
    pub ward: Option<String>,
}

/// A partner's boxes as sent (create and edit alike).
pub struct PartnerInput {
    pub name: String,
    pub kind: String,
    pub contact_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

/// A contract as sent. A box left empty or not parsed is `None`.
pub struct ContractInput {
    /// The area as text (`admin_units::AreaKey`); ignored on an edit.
    pub area: Option<String>,
    pub starts_on: Option<chrono::NaiveDate>,
    pub ends_on: Option<chrono::NaiveDate>,
    pub monthly_fee: Option<i64>,
    pub note: Option<String>,
    pub category_ids: Vec<i32>,
}

/// A crop's boxes (admin create and edit).
pub struct CropInput {
    pub name: String,
    /// The commodity it is of; `None` when none was chosen.
    pub commodity_id: Option<i32>,
    pub unit: String,
    pub display_order: i32,
}

/// A price as entered: a date that did not parse is `None` and refused like
/// a missing one; the same crop, area and day again replaces the price.
pub struct PriceInput {
    pub crop_id: Option<i32>,
    pub location_id: Option<i32>,
    pub observed_on: Option<chrono::NaiveDate>,
    pub price_low: Option<i64>,
    pub price_high: Option<i64>,
    pub note: Option<String>,
}

/// A provider's sign-up (`AuthUseCase::register`).
pub struct RegisterInput {
    pub email: String,
    pub password: String,
    pub full_name: Option<String>,
    pub business_name: String,
    pub business_phone: String,
    /// Who signs up, to cap tries per client (`SIGNUP_ATTEMPTS_PER_CLIENT`):
    /// the address, kept in memory as a hash only; empty for none known.
    pub client: String,
}

/// A farmer's job post: where is a position (`lat`/`lng`, from the phone)
/// or else a location preset.
pub struct JobInput {
    pub category_id: Option<i32>,
    pub location_id: Option<i32>,
    /// A ward typed in its box (`w28439`): its centre, when the phone gave
    /// no position.
    pub ward: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub description: String,
    pub requester_name: String,
    pub requester_phone: String,
    /// Who posts, to cap posts per client (`JOB_POSTS_PER_CLIENT_PER_DAY`):
    /// the address, kept in memory as a hash only; empty for none known.
    pub client: String,
}

/// An alert as typed (admin create and edit): a date that did not parse is
/// `None` and refused like a missing one; a blank sector or province means
/// every sector, the whole region.
pub struct AlertInput {
    pub kind: String,
    pub sector: String,
    pub province_code: String,
    pub title: String,
    pub body: String,
    pub starts_on: Option<chrono::NaiveDate>,
    pub ends_on: Option<chrono::NaiveDate>,
}

/// An information page's boxes as typed (create and edit; the code is only
/// read when the page is made, and a blank one is made from the title).
#[derive(Debug, Clone, Default)]
pub struct InfoPageInput {
    pub code: String,
    pub title: String,
    pub icon: String,
    pub in_footer: bool,
    pub footer_group: String,
    pub display_order: Option<i32>,
}

/// A new version of a page's text as typed: in force from `effective_on`
/// (none: today).
#[derive(Debug, Clone, Default)]
pub struct InfoRevisionInput {
    pub body: String,
    pub effective_on: Option<chrono::NaiveDate>,
    pub change_note: String,
}
