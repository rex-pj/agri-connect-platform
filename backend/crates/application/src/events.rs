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
        inquiry_repository_trait::InquiryRepositoryTrait, job_repository_trait::JobRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
        service_repository_trait::ServiceRepositoryTrait,
    },
    Result,
};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    features::{Feature, Stage},
    ports::{EmailService, EventBus, FileStorage, InquiryNotice, JobNotice},
    settings::SettingsReader,
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
        (Change::Verification { verified: true }, _) => "verify_provider".into(),
        (Change::Verification { verified: false }, _) => "unverify_provider".into(),
        (Change::Visibility { visible: true }, Entity::Provider) => "activate_provider".into(),
        (Change::Visibility { visible: false }, Entity::Provider) => "deactivate_provider".into(),
        (Change::Visibility { visible: true }, Entity::User) => "activate_user".into(),
        (Change::Visibility { visible: false }, Entity::User) => "deactivate_user".into(),
        (Change::Visibility { visible: true }, _) => format!("show_{entity}"),
        (Change::Visibility { visible: false }, _) => format!("hide_{entity}"),
        (Change::Deleted, _) => format!("delete_{entity}"),
        (Change::RoleChanged { .. }, _) => format!("change_{entity}_role"),
        (Change::ImageReplaced { .. }, _) => format!("replace_{entity}_image"),
        (Change::ImageRemoved { .. }, _) => format!("remove_{entity}_image"),
        (Change::PermissionsChanged { .. }, _) => format!("change_{entity}_permissions"),
        (Change::AccountAssigned { .. }, _) => format!("assign_{entity}_account"),
        (Change::AccountUnlinked, _) => format!("unlink_{entity}_account"),
        (Change::Contract { change, .. }, _) => change.as_str().into(),
        (Change::Updated, _) => format!("update_{entity}"),
        (Change::StatusChanged { .. }, _) => format!("set_{entity}_status"),
        (Change::Created, _) => format!("create_{entity}"),
        (Change::Auth(what), _) => what.as_str().into(),
        (Change::SettingsChanged { .. }, _) => "change_settings".into(),
    }
}

#[async_trait]
impl EventSubscriber for AuditLogSubscriber {
    fn name(&self) -> &'static str {
        "audit_log"
    }

    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        // The audit log is what signed-in users changed; a visitor's contact
        // request is the inquiry itself, already stored.
        let Some(actor_id) = event.actor_id else {
            return Ok(());
        };
        let reason = match (&event.change, &event.reason) {
            (Change::RoleChanged { role }, Some(r)) => Some(format!("{role}: {r}")),
            (Change::RoleChanged { role }, None) => Some(role.clone()),
            (Change::AccountAssigned { username }, _) => Some(username.clone()),
            (Change::StatusChanged { status }, Some(r)) => Some(format!("{status}: {r}")),
            (Change::StatusChanged { status }, None) => Some(status.clone()),
            (Change::Contract { about, .. }, Some(r)) => Some(format!("{about}: {r}")),
            (Change::Contract { about, .. }, None) => Some(about.clone()),
            // "key: before → after; …: reason".
            (Change::SettingsChanged { changes, .. }, r) => {
                let diff = changes.join("; ");
                Some(match r {
                    Some(r) => format!("{diff}: {r}"),
                    None => diff,
                })
            }
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
                actor_id,
                &audit_action(event),
                event.entity.as_str(),
                event.entity_id,
                reason,
            )
            .await
    }
}

/// Removes images nothing points at any more: the one a product or service
/// just replaced, and the one of a product or service just deleted. A failure
/// leaves an orphaned object (logged by the bus), never a broken image on a
/// live page.
pub struct FileCleanupSubscriber {
    products: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    services: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
    files: Arc<dyn FileRepositoryTrait + Send + Sync>,
    storage: Arc<dyn FileStorage>,
}

impl FileCleanupSubscriber {
    pub fn new(
        products: Arc<dyn ProductRepositoryTrait + Send + Sync>,
        services: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
        files: Arc<dyn FileRepositoryTrait + Send + Sync>,
        storage: Arc<dyn FileStorage>,
    ) -> Self {
        Self {
            products,
            services,
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
        let unreferenced: Vec<Uuid> = match (&event.change, event.entity) {
            (Change::ImageReplaced { previous }, _) => previous.iter().copied().collect(),
            (Change::ImageRemoved { previous }, _) => vec![*previous],
            // A replaced or removed logo, browser icon.
            (Change::SettingsChanged { released, .. }, _) => released.clone(),
            // Both are soft-deleted, so the row still names its image.
            (Change::Deleted, Entity::Product) => self
                .products
                .image_of(event.entity_id)
                .await?
                .into_iter()
                .collect(),
            (Change::Deleted, Entity::Service) => self
                .services
                .image_of(event.entity_id)
                .await?
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        for id in unreferenced {
            release_file(self.files.as_ref(), self.storage.as_ref(), id).await?;
        }
        Ok(())
    }
}

/// Emails the provider when a farmer sends them an inquiry, so the "they will
/// call you back" on the contact form holds even when nobody opens the inbox.
/// The email goes out in the background: the farmer's form never waits on the
/// mail server, and a failed delivery is logged by the adapter.
pub struct InquiryNotificationSubscriber {
    inquiries: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    email: Arc<dyn EmailService>,
    /// The provider inbox page, from the web layer's route table.
    inbox_path: String,
}

impl InquiryNotificationSubscriber {
    pub fn new(
        inquiries: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
        email: Arc<dyn EmailService>,
        inbox_path: String,
    ) -> Self {
        Self {
            inquiries,
            email,
            inbox_path,
        }
    }
}

#[async_trait]
impl EventSubscriber for InquiryNotificationSubscriber {
    fn name(&self) -> &'static str {
        "inquiry_notification"
    }

    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        if !(event.entity == Entity::Inquiry && event.change == Change::Created) {
            return Ok(());
        }
        let Some(inquiry) = self.inquiries.find_for_notice(event.entity_id).await? else {
            return Ok(());
        };
        let Some(to) = inquiry.recipient_email else {
            info!(
                inquiry = event.entity_id,
                "no email address to notify the provider at"
            );
            return Ok(());
        };
        let notice = InquiryNotice {
            business_name: inquiry.business_name,
            requester_name: inquiry.requester_name,
            requester_phone: inquiry.requester_phone,
            topic: inquiry.topic,
            note: inquiry.note,
            inbox_path: self.inbox_path.clone(),
        };
        let email = Arc::clone(&self.email);
        tokio::spawn(async move { email.send_new_inquiry(&to, &notice).await });
        Ok(())
    }
}

/// Emails the providers near a new job post that offer its category (the
/// nearest `JOB_NOTIFY_MAX` within `JOB_NOTIFY_RADIUS_M`), in a background
/// task: the farmer's page never waits for the mail server.
pub struct JobNotificationSubscriber {
    jobs: Arc<dyn JobRepositoryTrait + Send + Sync>,
    email: Arc<dyn EmailService>,
    /// How far and how many (`jobs.notify_radius_km`, `jobs.notify_max`).
    settings: SettingsReader,
    /// The provider's jobs page, from the web layer's route table.
    jobs_path: String,
}

impl JobNotificationSubscriber {
    pub fn new(
        jobs: Arc<dyn JobRepositoryTrait + Send + Sync>,
        email: Arc<dyn EmailService>,
        settings: SettingsReader,
        jobs_path: String,
    ) -> Self {
        Self {
            jobs,
            email,
            settings,
            jobs_path,
        }
    }
}

#[async_trait]
impl EventSubscriber for JobNotificationSubscriber {
    fn name(&self) -> &'static str {
        "job_notification"
    }

    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        if !(event.entity == Entity::JobPost && event.change == Change::Created) {
            return Ok(());
        }
        let settings = self.settings.load().await?;
        // A post made while hire requests are in preview (an admin trying
        // them) reaches no real provider.
        if settings.stage(Feature::Jobs) != Stage::On {
            info!(
                job = event.entity_id,
                "hire requests in preview; no provider told"
            );
            return Ok(());
        }
        let Some(job) = self
            .jobs
            .find_for_notice(
                event.entity_id,
                settings.job_notify_radius_m(),
                settings.job_notify_max(),
            )
            .await?
        else {
            return Ok(());
        };
        info!(
            job = event.entity_id,
            providers = job.recipients.len(),
            "telling nearby providers about a new job"
        );
        let email = Arc::clone(&self.email);
        let jobs_path = self.jobs_path.clone();
        tokio::spawn(async move {
            for to in job.recipients {
                let notice = JobNotice {
                    business_name: to.business_name,
                    category: job.category.clone(),
                    area: job.area.clone(),
                    description: job.description.clone(),
                    requester_name: job.requester_name.clone(),
                    requester_phone: job.requester_phone.clone(),
                    jobs_path: jobs_path.clone(),
                };
                email.send_new_job(&to.email, &notice).await;
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(entity: Entity, change: Change) -> DomainEvent {
        DomainEvent {
            actor_id: Some(1),
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
                Change::Verification { verified: true }
            )),
            "verify_provider"
        );
        assert_eq!(
            audit_action(&event(
                Entity::Provider,
                Change::Visibility { visible: false }
            )),
            "deactivate_provider"
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
