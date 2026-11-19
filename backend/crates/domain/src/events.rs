//! Things that happened, published after the change is stored. Subscribers
//! (today the audit log; later notifications and webhooks) react to them, so a
//! use case never has to know who is listening. This is where plugins would
//! attach.

use uuid::Uuid;

/// What kind of record an event is about. The wire names are stored in
/// `audit_logs.entity_type`, so renaming one splits that history in two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entity {
    Provider,
    User,
    Category,
    Review,
    Location,
    Product,
    Service,
    Role,
    Inquiry,
    Partner,
    Contract,
    Commodity,
    Sector,
    AlertKind,
    Crop,
    CropPrice,
    JobPost,
    Alert,
    /// The platform settings (one record: `entity_id` 0).
    Setting,
}

impl Entity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Entity::Provider => "provider",
            Entity::User => "user",
            Entity::Category => "category",
            Entity::Review => "review",
            Entity::Location => "location",
            Entity::Product => "product",
            Entity::Service => "service",
            Entity::Role => "role",
            Entity::Inquiry => "inquiry",
            Entity::Partner => "partner",
            Entity::Contract => "contract",
            Entity::Commodity => "commodity",
            Entity::Sector => "sector",
            Entity::AlertKind => "alert_kind",
            Entity::Crop => "crop",
            Entity::CropPrice => "crop_price",
            Entity::JobPost => "job_post",
            Entity::Alert => "alert",
            Entity::Setting => "setting",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Shown/hidden, activated/deactivated.
    Visibility {
        visible: bool,
    },
    /// A provider's "verified" badge was granted or withdrawn.
    Verification {
        verified: bool,
    },
    Deleted,
    RoleChanged {
        role: String,
    },
    /// A product or service got a new image. `previous` is no longer referenced by it and
    /// can be removed from storage.
    ImageReplaced {
        previous: Option<Uuid>,
    },
    /// A moderator took the image off a product or service; `previous` is
    /// no longer referenced and can be removed from storage.
    ImageRemoved {
        previous: Uuid,
    },
    /// A role now grants `added` and no longer grants `removed` (permission keys).
    PermissionsChanged {
        added: Vec<String>,
        removed: Vec<String>,
    },
    /// A business is now managed by the provider account `username`.
    AccountAssigned {
        username: String,
    },
    /// A business no longer has a managing account (the account stays).
    AccountUnlinked,
    /// Its fields were edited (a category, a location preset).
    Updated,
    /// An inquiry moved to `status` (contacted, closed, spam…).
    StatusChanged {
        status: String,
    },
    /// It was just made (an inquiry a farmer sent).
    Created,
    /// Settings changed, each as "key: before → after".
    SettingsChanged {
        changes: Vec<String>,
    },
    /// Something about signing in to an account; the actor is the account.
    Auth(AuthEvent),
}

/// What happened to an account's sign-in, for the audit log: who got in,
/// who was kept out, and when a password changed hands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthEvent {
    LoggedIn,
    /// A wrong password for this account.
    LoginFailed,
    PasswordChanged,
    /// The password was set through an emailed reset link.
    PasswordReset,
    /// A spent refresh token was shown again: a copy of the session exists,
    /// and the session was ended.
    SessionReused,
    EmailVerified,
}

impl AuthEvent {
    /// The audit log's action.
    pub const fn as_str(self) -> &'static str {
        match self {
            AuthEvent::LoggedIn => "login",
            AuthEvent::LoginFailed => "login_failed",
            AuthEvent::PasswordChanged => "change_password",
            AuthEvent::PasswordReset => "reset_password",
            AuthEvent::SessionReused => "refresh_token_reused",
            AuthEvent::EmailVerified => "verify_email",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEvent {
    /// The signed-in user who made the change; `None` for a visitor (a
    /// farmer's contact request).
    pub actor_id: Option<i32>,
    pub entity: Entity,
    pub entity_id: i32,
    pub change: Change,
    /// Free text the admin gave, if any.
    pub reason: Option<String>,
}
