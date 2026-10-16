//! In-process event bus. Use cases publish a `DomainEvent` after the change is
//! stored; every subscriber sees it. Subscriber failures are logged and never
//! undo or fail the change that was already made.

use std::sync::Arc;

use async_trait::async_trait;
use domain::{
    events::{Change, DomainEvent, Entity},
    repositories::{
        audit_log_repository_trait::AuditLogRepositoryTrait,
        file_repository_trait::FileRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
    },
    Result,
};
use tracing::warn;

use crate::{
    ports::{EventBus, FileStorage},
    usecases::media_usecase::release_file,
};

#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// For log lines when this subscriber fails.
    fn name(&self) -> &'static str;
    async fn handle(&self, event: &DomainEvent) -> Result<()>;
}

/// Delivers synchronously, in registration order, before `publish` returns.
/// Enough for one process; a queue would replace it behind the same port.
pub struct InProcessEventBus {
    subscribers: Vec<Arc<dyn EventSubscriber>>,
}

impl InProcessEventBus {
    pub fn new(subscribers: Vec<Arc<dyn EventSubscriber>>) -> Self {
        Self { subscribers }
    }
}

#[async_trait]
impl EventBus for InProcessEventBus {
    async fn publish(&self, event: DomainEvent) {
        for subscriber in &self.subscribers {
            if let Err(e) = subscriber.handle(&event).await {
                warn!(subscriber = subscriber.name(), entity = event.entity.as_str(), entity_id = event.entity_id, error = %e, "event subscriber failed");
            }
        }
    }
}

/// Writes one `audit_logs` row per event. The action names are stored, so
/// renaming one splits that action's history.
pub struct AuditLogSubscriber {
    repo: Arc<dyn AuditLogRepositoryTrait + Send + Sync>,
}

impl AuditLogSubscriber {
    pub fn new(repo: Arc<dyn AuditLogRepositoryTrait + Send + Sync>) -> Self {
        Self { repo }
    }
}

pub fn audit_action(event: &DomainEvent) -> String {
    let entity = event.entity.as_str();
    match (&event.change, event.entity) {
        (Change::Visibility { visible: true }, Entity::Provider) => "verify_provider".into(),
        (Change::Visibility { visible: false }, Entity::Provider) => "unverify_provider".into(),
        (Change::Visibility { visible: true }, Entity::User) => "activate_user".into(),
        (Change::Visibility { visible: false }, Entity::User) => "deactivate_user".into(),
        (Change::Visibility { visible: true }, _) => format!("show_{entity}"),
        (Change::Visibility { visible: false }, _) => format!("hide_{entity}"),
        (Change::Deleted, _) => format!("delete_{entity}"),
        (Change::RoleChanged { .. }, _) => format!("change_{entity}_role"),
        (Change::ImageReplaced { .. }, _) => format!("replace_{entity}_image"),
        (Change::PermissionsChanged { .. }, _) => format!("change_{entity}_permissions"),
        (Change::AccountAssigned { .. }, _) => format!("assign_{entity}_account"),
        (Change::Updated, _) => format!("update_{entity}"),
        (Change::StatusChanged { .. }, _) => format!("set_{entity}_status"),
    }
}

#[async_trait]
impl EventSubscriber for AuditLogSubscriber {
    fn name(&self) -> &'static str {
        "audit_log"
    }

    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let reason = match (&event.change, &event.reason) {
            (Change::RoleChanged { role }, Some(r)) => Some(format!("{role}: {r}")),
            (Change::RoleChanged { role }, None) => Some(role.clone()),
            (Change::AccountAssigned { username }, _) => Some(username.clone()),
            (Change::StatusChanged { status }, Some(r)) => Some(format!("{status}: {r}")),
            (Change::StatusChanged { status }, None) => Some(status.clone()),
            // The audit row says what changed: "+key, -key: reason".
            (Change::PermissionsChanged { added, removed }, r) => {
                let diff = added
                    .iter()
                    .map(|k| format!("+{k}"))
                    .chain(removed.iter().map(|k| format!("-{k}")))
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(match r {
                    Some(r) => format!("{diff}: {r}"),
                    None => diff,
                })
            }
            (_, r) => r.clone(),
        };
        self.repo
            .log(
                event.actor_id,
                &audit_action(event),
                event.entity.as_str(),
                event.entity_id,
                reason,
            )
            .await
    }
}

/// Removes images nothing points at any more: the one a product just
/// replaced, and the one of a product just deleted. A failure leaves an
/// orphaned object (logged by the bus), never a broken image on a live page.
pub struct FileCleanupSubscriber {
    products: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    files: Arc<dyn FileRepositoryTrait + Send + Sync>,
    storage: Arc<dyn FileStorage>,
}

impl FileCleanupSubscriber {
    pub fn new(
        products: Arc<dyn ProductRepositoryTrait + Send + Sync>,
        files: Arc<dyn FileRepositoryTrait + Send + Sync>,
        storage: Arc<dyn FileStorage>,
    ) -> Self {
        Self {
            products,
            files,
            storage,
        }
    }
}

#[async_trait]
impl EventSubscriber for FileCleanupSubscriber {
    fn name(&self) -> &'static str {
        "file_cleanup"
    }

    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let unreferenced = match (&event.change, event.entity) {
            (Change::ImageReplaced { previous }, _) => *previous,
            // Products are soft-deleted, so the row still names its image.
            (Change::Deleted, Entity::Product) => self.products.image_of(event.entity_id).await?,
            _ => None,
        };
        match unreferenced {
            Some(id) => release_file(self.files.as_ref(), self.storage.as_ref(), id).await,
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(entity: Entity, change: Change) -> DomainEvent {
        DomainEvent {
            actor_id: 1,
            entity,
            entity_id: 2,
            change,
            reason: None,
        }
    }

    #[test]
    fn action_names_match_the_existing_audit_history() {
        // These strings were written by the pre-event code; keeping them keeps
        // one continuous history per action.
        assert_eq!(
            audit_action(&event(
                Entity::Provider,
                Change::Visibility { visible: true }
            )),
            "verify_provider"
        );
        assert_eq!(
            audit_action(&event(Entity::User, Change::Visibility { visible: false })),
            "deactivate_user"
        );
        assert_eq!(
            audit_action(&event(
                Entity::Category,
                Change::Visibility { visible: false }
            )),
            "hide_category"
        );
        assert_eq!(
            audit_action(&event(Entity::Location, Change::Deleted)),
            "delete_location"
        );
        assert_eq!(
            audit_action(&event(Entity::Review, Change::Visibility { visible: true })),
            "show_review"
        );
    }

    #[test]
    fn edits_and_inquiry_handling_have_their_own_actions() {
        assert_eq!(
            audit_action(&event(Entity::Category, Change::Updated)),
            "update_category"
        );
        let handled = event(
            Entity::Inquiry,
            Change::StatusChanged {
                status: "contacted".into(),
            },
        );
        assert_eq!(audit_action(&handled), "set_inquiry_status");
    }
}
