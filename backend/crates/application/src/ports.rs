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
    pub username: String,
    /// Expiry as a Unix timestamp in seconds.
    pub expires_at: i64,
    /// When it was minted, Unix seconds; 0 for tokens minted before the
    /// claim existed (they are older than any cut-off).
    pub issued_at: i64,
}

/// Mint and verify signed session tokens.
pub trait TokenService: Send + Sync {
    fn mint(&self, user_id: i32, username: &str) -> Result<String>;

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
}
