use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    error::Error,
    models::job::{JobForNotice, JobPost, NewJobPost},
};

#[async_trait]
pub trait JobRepositoryTrait {
    /// Its id.
    async fn create(&self, job: &NewJobPost) -> Result<i32, Error>;

    /// Open posts from `phone` since `since`: the per-phone cap.
    async fn count_open_by_phone(&self, phone: &str, since: DateTime<Utc>) -> Result<u64, Error>;

    /// A job and the providers within `radius_m` of it with a live service
    /// in its category (live, not demo, with an address to write to),
    /// nearest first, at most `limit`.
    async fn find_for_notice(
        &self,
        id: i32,
        radius_m: f64,
        limit: i64,
    ) -> Result<Option<JobForNotice>, Error>;

    /// Open posts since `since` within `radius_m` of the business the
    /// account `owner_user` manages, in the categories of its live services,
    /// newest first. Empty when it has no business, no position or no
    /// service.
    async fn open_near_owner(
        &self,
        owner_user: i32,
        since: DateTime<Utc>,
        radius_m: f64,
        limit: i64,
    ) -> Result<Vec<JobPost>, Error>;

    /// A page of every post, newest first.
    async fn list_admin(&self, limit: i64, offset: i64) -> Result<Vec<JobPost>, Error>;

    /// How many posts `list_admin` pages through.
    async fn count_admin(&self) -> Result<i64, Error>;

    /// False when there is no such post.
    async fn set_status(&self, id: i32, status: &str) -> Result<bool, Error>;

    /// Deletes the posts made before `before`, whatever their status; how many.
    async fn delete_older_than(&self, before: DateTime<Utc>) -> Result<u64, Error>;
}
