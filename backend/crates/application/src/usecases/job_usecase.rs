//! Job posts: a farmer who found no one (or would rather be called) says
//! what needs doing, in which service category and where; the providers
//! nearby that offer it are emailed (`JobNotificationSubscriber`) and see it
//! on their jobs page with the farmer's phone. A post is never public.
//!
//! Where is the phone's position when the farmer shared it, else a location
//! preset. A post is open to providers for the `jobs.open_days` setting; one
//! phone has at most `jobs.open_per_phone` open at once, one client sends at
//! most `jobs.posts_per_client_per_day` a day, and while the
//! hire-request feature (`features::Feature::Jobs`) is off none is taken (`feature_off`).
//! The admin closes a post or marks it spam (`job.moderate`).

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use domain::{
    constants::{CATEGORY_TYPE_SERVICE, JOB_STATUSES},
    events::{Change, DomainEvent, Entity},
    models::admin_list::AdminFilter,
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
        ADMIN_PAGE_SIZE, JOB_DESCRIPTION_MAX, JOB_POST_CLIENT_WINDOW_SECS, JOB_POST_KEEP_DAYS,
        PAGE_SIZE, REQUESTER_NAME_MAX_LEN,
    },
    features::Feature,
    hashing::sha256_hex,
    inputs::JobInput,
    outputs::{AdminPagedResponse, JobPostDto, NamedDto, PagedResponse},
    pagination::{paginate_admin, paginate_public, total_or_count},
    ports::EventBus,
    repeats::WindowCounter,
    settings::SettingsReader,
    usecases::place_usecase::PlaceUseCase,
    validation::{
        audit_reason, max_chars, point_wkt, typed_state, validate_coordinates, validate_phone,
    },
};

#[derive(Clone)]
pub struct JobUseCase {
    job_repo: Arc<dyn JobRepositoryTrait + Send + Sync>,
    category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    events: Arc<dyn EventBus>,
    settings: SettingsReader,
    /// The centre of a ward chosen instead of a preset.
    places: PlaceUseCase,
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
        places: PlaceUseCase,
    ) -> Self {
        Self {
            job_repo,
            category_repo,
            location_repo,
            events,
            settings,
            places,
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
    /// hire requests are off (`feature_off`).
    pub async fn post(&self, input: JobInput) -> Result<i32> {
        let settings = self.settings.load().await?;
        if !settings.is_open(Feature::Jobs) {
            return Err(Error::new(ErrorCode::FeatureOff));
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
        // A ward typed in its box: its centre (a database failure is no
        // farmer's mistake and fails the post).
        let ward = match self.places.ward_centre(input.ward.as_deref()).await {
            Err(e) if e.code() != ErrorCode::LocationNotFound => return Err(e),
            ward => ward,
        };
        // The phone's position when it is a real one, else the ward's, else
        // the preset's.
        let position = match (input.lat, input.lng) {
            (Some(lat), Some(lng)) if validate_coordinates(lat, lng).is_ok() => Some((lat, lng)),
            _ => ward
                .as_ref()
                .ok()
                .copied()
                .flatten()
                .or_else(|| preset.map(|l| (l.lat, l.lng))),
        };
        Validation::new()
            .check(
                category.is_none(),
                "category_id",
                ErrorCode::InvalidServiceCategory,
            )
            .rule("ward", ward.map(|_| ()))
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
        // The client's slot for the day is taken now, checked and counted
        // in one step (the check above answers early), and given back if
        // the post is not stored: posts sent at once cannot all pass.
        if let Some(key) = &client_key {
            if !self
                .posts_by_client
                .take(key, window, settings.job_posts_per_client_per_day())
            {
                return Err(Error::new(ErrorCode::JobPostDailyLimit));
            }
        }
        let since = Utc::now() - Duration::days(settings.job_post_days());
        let created = self
            .job_repo
            .create_capped(
                &NewJobPost {
                    category_id,
                    location_id: preset.map(|l| l.id),
                    point_wkt: point_wkt(lat, lng),
                    description,
                    requester_name,
                    requester_phone,
                },
                since,
                settings.job_posts_open_per_phone(),
            )
            .await;
        let id = match created {
            Ok(Some(id)) => id,
            refused => {
                if let Some(key) = &client_key {
                    self.posts_by_client.give_back(key);
                }
                return Err(match refused {
                    Err(e) => e,
                    Ok(_) => Error::field("requester_phone", ErrorCode::JobPostLimit),
                });
            }
        };
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
    /// services: one page of them, newest first.
    pub async fn list_own(&self, actor: &Actor, page: i64) -> Result<PagedResponse<JobPostDto>> {
        actor.require(PERM_OWN_JOB_READ)?;
        let settings = self.settings.load().await?;
        let since = Utc::now() - Duration::days(settings.job_post_days());
        let radius_m = settings.job_notify_radius_m();
        let (page, offset) = paginate_public(page);
        let rows = self
            .job_repo
            .open_near_owner(actor.user_id, since, radius_m, PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, rows.len(), PAGE_SIZE, || {
            self.job_repo
                .count_open_near_owner(actor.user_id, since, radius_m)
        })
        .await?;
        let items = rows.into_iter().map(job_dto).collect();
        Ok(PagedResponse::public(items, total, page))
    }

    /// How many posts the provider's jobs page lists, all its pages: the
    /// dashboard's link to it.
    pub async fn count_own(&self, actor: &Actor) -> Result<i64> {
        actor.require(PERM_OWN_JOB_READ)?;
        let settings = self.settings.load().await?;
        let since = Utc::now() - Duration::days(settings.job_post_days());
        self.job_repo
            .count_open_near_owner(actor.user_id, since, settings.job_notify_radius_m())
            .await
    }

    /// How many posts are in `status`: the count the list's chip of that
    /// status shows.
    pub async fn count_admin(&self, actor: &Actor, status: &str) -> Result<i64> {
        actor.require(PERM_JOB_MODERATE)?;
        if !JOB_STATUSES.contains(&status) {
            return Err(Error::field("status", ErrorCode::InvalidJobStatus));
        }
        let state = JOB_STATUSES.iter().copied().find(|s| *s == status);
        self.job_repo
            .count_admin(&AdminFilter::default().in_state(state))
            .await
    }

    /// The service types the post list can be narrowed to: every service
    /// type, hidden ones too (a post may name one since hidden).
    pub async fn service_type_choices(&self, actor: &Actor) -> Result<Vec<NamedDto>> {
        actor.require(PERM_JOB_MODERATE)?;
        Ok(self
            .category_repo
            .list_admin()
            .await?
            .into_iter()
            .filter(|c| c.category_type == CATEGORY_TYPE_SERVICE)
            .map(|c| NamedDto {
                id: c.id,
                name: c.name,
            })
            .collect())
    }

    /// The areas the post list can be narrowed to: every area, hidden ones
    /// too (a post may name one since hidden).
    pub async fn area_choices(&self, actor: &Actor) -> Result<Vec<NamedDto>> {
        actor.require(PERM_JOB_MODERATE)?;
        Ok(self
            .location_repo
            .list_all()
            .await?
            .into_iter()
            .map(|l| NamedDto {
                id: l.id,
                name: l.label,
            })
            .collect())
    }

    /// The status a list of posts is narrowed to, as typed: one of
    /// `JOB_STATUSES`, blank for every one, else refused.
    pub fn job_state(typed: Option<&str>) -> Result<Option<&'static str>> {
        typed_state(JOB_STATUSES, typed, ErrorCode::InvalidJobStatus)
    }

    /// Every post (`filter`: one of `JOB_STATUSES`, the farmer's name or
    /// phone or the description, an order), newest first by default.
    pub async fn list_admin(
        &self,
        actor: &Actor,
        page: i64,
        filter: &AdminFilter,
    ) -> Result<AdminPagedResponse<JobPostDto>> {
        actor.require(PERM_JOB_MODERATE)?;
        if filter.state.is_some_and(|s| !JOB_STATUSES.contains(&s)) {
            return Err(Error::field("status", ErrorCode::InvalidJobStatus));
        }
        let (page, offset) = paginate_admin(page);
        let rows = self
            .job_repo
            .list_admin(filter, ADMIN_PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, rows.len(), ADMIN_PAGE_SIZE, || {
            self.job_repo.count_admin(filter)
        })
        .await?;
        let items = rows.into_iter().map(job_dto).collect();
        Ok(AdminPagedResponse::admin(items, total, page))
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
