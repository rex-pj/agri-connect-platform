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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Shown/hidden, verified/unverified, activated/deactivated.
    Visibility {
        visible: bool,
    },
    Deleted,
    RoleChanged {
        role: String,
    },
    /// A product got a new image. `previous` is no longer referenced by it and
    /// can be removed from storage.
    ImageReplaced {
        previous: Option<Uuid>,
    },
    /// A role now grants `added` and no longer grants `removed` (permission keys).
    PermissionsChanged {
        added: Vec<String>,
        removed: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEvent {
    /// The user who made the change.
    pub actor_id: i32,
    pub entity: Entity,
    pub entity_id: i32,
    pub change: Change,
    /// Free text the admin gave, if any.
    pub reason: Option<String>,
}
