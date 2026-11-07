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
    /// A role now grants `added` and no longer grants `removed` (permission keys).
    PermissionsChanged {
        added: Vec<String>,
        removed: Vec<String>,
    },
    /// A business is now managed by the provider account `username`.
    AccountAssigned {
        username: String,
    },
    /// Its fields were edited (a category, a location preset).
    Updated,
    /// An inquiry moved to `status` (contacted, closed, spam…).
    StatusChanged {
        status: String,
    },
    /// It was just made (an inquiry a farmer sent).
    Created,
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
