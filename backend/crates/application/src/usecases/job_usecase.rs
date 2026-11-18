//! Job posts: a farmer who found no one (or would rather be called) says
//! what needs doing, in which service category and where; the providers
//! nearby that offer it are emailed (`JobNotificationSubscriber`) and see it
//! on their jobs page with the farmer's phone. A post is never public.
//!
//! Where is the phone's position when the farmer shared it, else a location
//! preset. A post is open to providers for the `jobs.open_days` setting; one
//! phone has at most `jobs.open_per_phone` open at once, one client sends at
//! most `jobs.posts_per_client_per_day` a day, and while the
//! `access.job_posts_open` switch is off none is taken (`job_posts_paused`).
//! The admin closes a post or marks it spam (`job.moderate`).

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use domain::{
    constants::{CATEGORY_TYPE_SERVICE, JOB_STATUSES},
    events::{Change, DomainEvent, Entity},
    models::job::{JobPost, NewJobPost},
    repositories::{
        category_repository_trait::CategoryRepositoryTrait,
        job_repository_trait::JobRepositoryTrait,
        location_repository_trait::LocationRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::{PERM_JOB_MODERATE, PERM_OWN_JOB_READ},
    constants::{
        ADMIN_PAGE_SIZE, JOBS_SHOWN, JOB_DESCRIPTION_MAX, JOB_POST_CLIENT_WINDOW_SECS,
        JOB_POST_KEEP_DAYS, REQUESTER_NAME_MAX_LEN,
    },
    hashing::sha256_hex,
    inputs::JobInput,
    outputs::{AdminPagedResponse, JobPostDto},
    pagination::{admin_total_pages, paginate_admin, total_or_count},
    ports::EventBus,
    repeats::WindowCounter,
    settings::SettingsReader,
    validation::{audit_reason, max_chars, point_wkt, validate_coordinates, validate_phone},
};

#[derive(Clone)]
pub struct JobUseCase {
    job_repo: Arc<dyn JobRepositoryTrait + Send + Sync>,
    category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    events: Arc<dyn EventBus>,
    settings: SettingsReader,
    /// Posts per client in the last day, in memory only (never who).
    posts_by_client: WindowCounter,
}

impl JobUseCase {
    pub fn new(
        job_repo: Arc<dyn JobRepositoryTrait + Send + Sync>,
        category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
        location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
        events: Arc<dyn EventBus>,
        settings: SettingsReader,
    ) -> Self {
        Self {
            job_repo,
            category_repo,
            location_repo,
            events,
            settings,
            posts_by_client: WindowCounter::default(),
        }
    }

    /// Forgets the clients whose day has passed (bounded memory); called
    /// with the other in-memory filters.
    pub fn forget_old_posters(&self) {
        self.posts_by_client
            .forget_older_than(std::time::Duration::from_secs(JOB_POST_CLIENT_WINDOW_SECS));
    }

    /// A farmer's post; its id. Every bad box is reported at once. One
    /// client posts at most the `jobs.posts_per_client_per_day` setting a
    /// day, whatever phones it types (`job_post_daily_limit`); none while
    /// job posts are paused (`job_posts_paused`).
    pub async fn post(&self, input: JobInput) -> Result<i32> {
        let settings = self.settings.load().await?;
        if !settings.job_posts_open() {
            return Err(Error::new(ErrorCode::JobPostsPaused));
        }
        let window = std::time::Duration::from_secs(JOB_POST_CLIENT_WINDOW_SECS);
        // Hashed: the counter never holds an address.
        let client_key = (!input.client.is_empty()).then(|| sha256_hex(&input.client));
        if client_key.as_deref().is_some_and(|k| {
            self.posts_by_client.count(k, window) >= settings.job_posts_per_client_per_day()
        }) {
            return Err(Error::new(ErrorCode::JobPostDailyLimit));
        }
        let description = input.description.trim().to_owned();
        let requester_name = input.requester_name.trim().to_owned();
        let requester_phone = input.requester_phone.trim().to_owned();
        let (categories, locations) = tokio::try_join!(
            self.category_repo.list_admin(),
            self.location_repo.list_active()
        )?;
        let category = input.category_id.filter(|id| {
            categories
                .iter()
                .any(|c| c.id == *id && c.is_active && c.category_type == CATEGORY_TYPE_SERVICE)
        });
        let preset = input
            .location_id
            .and_then(|id| locations.iter().find(|l| l.id == id));
        // The phone's position when it is a real one, else the preset's.
        let position = match (input.lat, input.lng) {
            (Some(lat), Some(lng)) if validate_coordinates(lat, lng).is_ok() => Some((lat, lng)),
            _ => preset.map(|l| (l.lat, l.lng)),
        };
        Validation::new()
            .check(
                category.is_none(),
                "category_id",
                ErrorCode::InvalidServiceCategory,
            )
            .check(
                position.is_none(),
                "location_id",
                ErrorCode::JobPlaceRequired,
            )
            .check(
                description.is_empty(),
                "description",
                ErrorCode::JobDescriptionRequired,
            )
            .rule("description", max_chars(&description, JOB_DESCRIPTION_MAX))
            .check(
                requester_name.is_empty(),
                "requester_name",
                ErrorCode::RequesterNameRequired,
            )
            .rule(
                "requester_name",
                max_chars(&requester_name, REQUESTER_NAME_MAX_LEN),
            )
            .rule("requester_phone", validate_phone(&requester_phone))
            .finish()?;
        let (Some(category_id), Some((lat, lng))) = (category, position) else {
            return Err(Error::new(ErrorCode::ValidationFailed));
        };
        let since = Utc::now() - Duration::days(settings.job_post_days());
        if self
            .job_repo
            .count_open_by_phone(&requester_phone, since)
            .await?
            >= settings.job_posts_open_per_phone()
        {
            return Err(Error::field("requester_phone", ErrorCode::JobPostLimit));
        }
        let id = self
            .job_repo
            .create(&NewJobPost {
                category_id,
                location_id: preset.map(|l| l.id),
                point_wkt: point_wkt(lat, lng),
                description,
                requester_name,
                requester_phone,
            })
            .await?;
        if let Some(key) = client_key {
            self.posts_by_client.hit(key, window);
        }
        // A visitor's event: no actor, not audited; the subscriber emails.
        self.events
            .publish(DomainEvent {
                actor_id: None,
                entity: Entity::JobPost,
                entity_id: id,
                change: Change::Created,
                reason: None,
            })
            .await;
        Ok(id)
    }

    /// The provider's jobs page: posts still open (`jobs.open_days`) within
    /// the notice radius of its business, in the categories of its live
    /// services.
    pub async fn list_own(&self, actor: &Actor) -> Result<Vec<JobPostDto>> {
        actor.require(PERM_OWN_JOB_READ)?;
        let settings = self.settings.load().await?;
        let since = Utc::now() - Duration::days(settings.job_post_days());
        Ok(self
            .job_repo
            .open_near_owner(
                actor.user_id,
                since,
                settings.job_notify_radius_m(),
                JOBS_SHOWN,
            )
            .await?
            .into_iter()
            .map(job_dto)
            .collect())
    }

    /// How many posts are in `status`: the count the list's chip of that
    /// status shows.
    pub async fn count_admin(&self, actor: &Actor, status: &str) -> Result<i64> {
        actor.require(PERM_JOB_MODERATE)?;
        if !JOB_STATUSES.contains(&status) {
            return Err(Error::field("status", ErrorCode::InvalidJobStatus));
        }
        self.job_repo.count_admin(Some(status)).await
    }

    /// Every post (`status`: only those in it, one of `JOB_STATUSES`),
    /// newest first.
    pub async fn list_admin(
        &self,
        actor: &Actor,
        page: i64,
        status: Option<&str>,
    ) -> Result<AdminPagedResponse<JobPostDto>> {
        actor.require(PERM_JOB_MODERATE)?;
        if status.is_some_and(|s| !JOB_STATUSES.contains(&s)) {
            return Err(Error::field("status", ErrorCode::InvalidJobStatus));
        }
        let (page, offset) = paginate_admin(page);
        let rows = self
            .job_repo
            .list_admin(status, ADMIN_PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, rows.len(), ADMIN_PAGE_SIZE, || {
            self.job_repo.count_admin(status)
        })
        .await?;
        Ok(AdminPagedResponse {
            items: rows.into_iter().map(job_dto).collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: admin_total_pages(total),
        })
    }

    /// Deletes the posts older than `JOB_POST_KEEP_DAYS`, whatever their
    /// status; how many. Run now and then by the server: no page shows a
    /// post that old, and each holds a farmer's phone number.
    pub async fn purge_old_posts(&self, now: DateTime<Utc>) -> Result<u64> {
        self.job_repo
            .delete_older_than(now - Duration::days(JOB_POST_KEEP_DAYS))
            .await
    }

    /// Opens, closes or marks a post spam (`JOB_STATUSES`).
    pub async fn set_status(
        &self,
        actor: &Actor,
        id: i32,
        status: &str,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_JOB_MODERATE)?;
        let status = status.trim();
        if !JOB_STATUSES.contains(&status) {
            return Err(Error::field("status", ErrorCode::InvalidJobStatus));
        }
        if !self.job_repo.set_status(id, status).await? {
            return Err(Error::new(ErrorCode::JobNotFound));
        }
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity: Entity::JobPost,
                entity_id: id,
                change: Change::StatusChanged {
                    status: status.to_owned(),
                },
                reason: audit_reason(reason),
            })
            .await;
        Ok(())
    }
}

fn job_dto(j: JobPost) -> JobPostDto {
    JobPostDto {
        id: j.id,
        category_id: j.category_id,
        category: j.category,
        area: j.area,
        description: j.description,
        requester_name: j.requester_name,
        requester_phone: j.requester_phone,
        status: j.status,
        created_on: j.created_on,
        distance_m: j.distance_m,
    }
}
