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
    /// A business (or partner) is now managed by the account `email`.
    AccountAssigned {
        email: String,
    },
    /// A business no longer has a managing account (the account stays).
    AccountUnlinked,
    /// One of a partner's contracts changed: audited on the partner, the
    /// record whose page shows it (a contract has no page, and a deleted one
    /// is gone). `about` names it: its area, and the shop for a sponsorship.
    Contract {
        change: ContractChange,
        about: String,
    },
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
        /// Uploaded files the change stopped using (a replaced logo), for
        /// the file cleanup.
        released: Vec<Uuid>,
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
    /// A link sent to another address was opened: the account moved to it.
    EmailChanged,
    /// The account asked to move to another address (the link is sent).
    EmailChangeRequested,
    /// The account changed the name it is shown by.
    ProfileUpdated,
    /// The account signed out every other device.
    OtherSessionsEnded,
    /// An admin had a password-reset link sent to the account.
    PasswordResetSent,
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
            AuthEvent::EmailChanged => "change_email",
            AuthEvent::EmailChangeRequested => "request_email_change",
            AuthEvent::ProfileUpdated => "update_profile",
            AuthEvent::OtherSessionsEnded => "end_other_sessions",
            AuthEvent::PasswordResetSent => "send_password_reset",
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

/// What happened to one of a partner's contracts (`Change::Contract`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractChange {
    Added,
    Edited,
    Deleted,
    Sponsored,
    Unsponsored,
}

impl ContractChange {
    /// Its audit code, worded by `[text.audit]`.
    pub const fn as_str(self) -> &'static str {
        match self {
            ContractChange::Added => "add_partner_contract",
            ContractChange::Edited => "update_partner_contract",
            ContractChange::Deleted => "delete_partner_contract",
            ContractChange::Sponsored => "sponsor_partner_shop",
            ContractChange::Unsponsored => "unsponsor_partner_shop",
        }
    }
}
