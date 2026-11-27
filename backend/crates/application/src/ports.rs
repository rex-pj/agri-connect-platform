//! What the application needs from the outside world, without saying how.
//! Concrete adapters live in `infrastructure` and are chosen in `startup.rs`.

use async_trait::async_trait;
use domain::Result;

/// Send transactional emails. Delivery failures are logged by the adapter and
/// never surface to the caller: a password-reset request must look identical
/// whether or not the address exists.
#[async_trait]
pub trait EmailService: Send + Sync {
    async fn send_password_reset(&self, to_email: &str, token: &str);

    /// The link that confirms `to_email` is its account's address.
    async fn send_email_verification(&self, to_email: &str, token: &str);

    /// The link that moves an account to `to_email`, the address it asked
    /// for: nothing changes until it is opened.
    async fn send_email_change(&self, to_email: &str, token: &str);

    /// Tells the account's former address that it moved to `masked_email`
    /// (`mask_email`: enough for the owner to recognise, not to write to).
    async fn send_email_changed(&self, to_email: &str, masked_email: &str);

    /// Tells a provider a farmer asked to be called back.
    async fn send_new_inquiry(&self, to_email: &str, notice: &InquiryNotice);

    /// Tells a provider nearby that a farmer posted a job it can do.
    async fn send_new_job(&self, to_email: &str, notice: &JobNotice);
}

/// What the provider's "new inquiry" email says. The farmer typed the name,
/// topic and note: adapters must send them as plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InquiryNotice {
    pub business_name: String,
    pub requester_name: String,
    pub requester_phone: String,
    pub topic: String,
    pub note: Option<String>,
    /// The path of the provider's inbox page; the adapter puts its own
    /// origin (APP_URL) in front, as for reset links.
    pub inbox_path: String,
}

/// What the "a job near you" email says. The farmer typed the name and the
/// description: adapters must send them as plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobNotice {
    pub business_name: String,
    pub category: String,
    /// The area the farmer chose, if any.
    pub area: Option<String>,
    pub description: String,
    pub requester_name: String,
    pub requester_phone: String,
    /// The path of the provider's jobs page; the adapter puts its origin in
    /// front.
    pub jobs_path: String,
}

/// What a verified session token says about its holder. No role: what the
/// holder may do is read from the database on every request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionClaims {
    pub user_id: i32,
    /// Expiry as a Unix timestamp in seconds.
    pub expires_at: i64,
    /// When it was minted, Unix seconds; 0 for tokens minted before the
    /// claim existed (they are older than any cut-off).
    pub issued_at: i64,
    /// The session (refresh-token family) the token belongs to, so logging
    /// out, a password change or a deactivated account ends it at once. Every
    /// token has one, the JSON login's too: a token without is refused.
    pub session: uuid::Uuid,
}

/// Mint and verify signed session tokens.
pub trait TokenService: Send + Sync {
    /// `session`: the session (refresh-token family) the token is part of.
    fn mint(&self, user_id: i32, session: uuid::Uuid) -> Result<String>;

    /// Fails with `invalid_token` for a bad signature, a malformed token or an
    /// expired one; callers cannot tell those apart, by design.
    fn verify(&self, token: &str) -> Result<SessionClaims>;
}

/// Hash and verify passwords. Async because the adapter must run the CPU-heavy
/// work off the async worker threads.
#[async_trait]
pub trait PasswordHasher: Send + Sync {
    async fn hash(&self, password: &str) -> Result<String>;
    async fn verify(&self, password: &str, hash: &str) -> Result<bool>;
}

/// Publishes domain events to every subscriber (audit log today). Never fails
/// the caller: the change is already stored when an event is published.
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: domain::events::DomainEvent);
}

/// Fixed-window request budgets, per key (the client address).
pub trait RateLimiter: Send + Sync {
    /// `true` when this request is within the budget and has been counted.
    fn allow(&self, key: std::net::IpAddr) -> bool;

    /// Drops entries for clients whose window has fully refilled. Without it the
    /// table keeps one entry per address ever seen, forever.
    fn forget_idle(&self);
}

/// Where uploaded objects live (a bucket in production, a directory in
/// development). Keys are ours (`images/<uuid>.jpg`), never user input. The
/// content type is kept in the `files` row, not on the object.
#[async_trait]
pub trait FileStorage: Send + Sync {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<()>;

    /// Whole object in memory: stored images are re-encoded and capped at a
    /// few hundred KB, so streaming would buy nothing.
    async fn get(&self, key: &str) -> Result<Vec<u8>>;

    /// Deleting a missing key succeeds: cleanup may run twice.
    async fn delete(&self, key: &str) -> Result<()>;
}

/// One re-encoded image, ready to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedImage {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedImage {
    pub main: EncodedImage,
    pub thumb: EncodedImage,
    /// The encoder's type for both, e.g. `image/jpeg`.
    pub content_type: &'static str,
}

/// Turns untrusted upload bytes into images we are willing to serve. Fails
/// with `unsupported_image_type`, `image_too_many_pixels` or `invalid_image`.
/// Async because the adapter must run the decoding off the async workers.
#[async_trait]
pub trait ImageProcessor: Send + Sync {
    async fn process(&self, upload: Vec<u8>) -> Result<ProcessedImage>;

    /// The same checks for the platform's logo or browser icon, re-encoded
    /// as PNG squares (centred): the small one (`ICON_THUMB_SIDE`, the bars
    /// and the tab) with its transparency kept, the large one
    /// (`ICON_MAIN_SIDE`, the phone home screen) on white with a margin, as
    /// iOS paints transparency black. Never SVG (a script in
    /// an image served from our origin). Also `image_too_small` under
    /// `ICON_MIN_SIDE` on its shorter side.
    async fn process_icon(&self, upload: Vec<u8>) -> Result<ProcessedImage>;
}

/// The Zalo Graph API, for a Mini App's tokens: what a `getLocation` or
/// `getPhoneNumber` token stands for. Each token is single-use and expires
/// two minutes after it was issued; the server converts it with the app's
/// secret key, which never reaches the phone.
#[async_trait]
pub trait ZaloGateway: Send + Sync {
    /// (latitude, longitude) of a `getLocation` token.
    async fn location(&self, access_token: &str, code: &str) -> domain::Result<(f64, f64)>;

    /// The phone number of a `getPhoneNumber` token, as Zalo writes it
    /// ("849…").
    async fn phone_number(&self, access_token: &str, code: &str) -> domain::Result<String>;
}

/// The map's background pictures (OpenStreetMap tiles, `MAP_TILE_URL`),
/// fetched by the server so a page keeps `img-src 'self'` and the tile host
/// never sees a visitor.
#[async_trait]
pub trait MapTiles: Send + Sync {
    /// The PNG of tile `x`, `y` at zoom `z`, already checked to be one of
    /// the region's (`map_tile_not_found` when the host has none,
    /// `map_unavailable` when it cannot be reached).
    async fn tile(&self, z: u8, x: u32, y: u32) -> domain::Result<Vec<u8>>;
}

/// One heading of a rendered text: the outline a long page shows above it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// 2 or 3: a text's `#` is drawn as a `##`, since the page's title is
    /// its only `<h1>`.
    pub level: u8,
    /// The anchor of the heading (`#dieu-1-pham-vi`), unique in the text.
    pub id: String,
    pub text: String,
}

/// A text written in Markdown, made into HTML that is safe to put on a page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderedText {
    pub html: String,
    /// The `##` and `###` headings, in order.
    pub outline: Vec<Heading>,
}

/// Turns an admin's Markdown (an information page, a news post) into HTML.
/// No HTML written in the text ever reaches the page (it is shown as text),
/// a link leads only to `https:`, `http:`, `mailto:`, `tel:`, a path of
/// this site or an anchor, and an image is only one of our uploads
/// (`/files/…`); anything else is dropped, its words kept.
pub trait MarkdownRenderer: Send + Sync {
    fn render(&self, source: &str) -> RenderedText;
}
