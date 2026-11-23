use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    error::Error,
    models::{
        admin_list::AdminFilter,
        job::{JobForNotice, JobPost, NewJobPost},
    },
};

#[async_trait]
pub trait JobRepositoryTrait {
    /// Its id, unless its phone already has `max` open posts since `since`
    /// (`None`; demo posts never count). Counted and inserted under a lock
    /// on the phone, so two posts sent at once cannot both pass the cap.
    async fn create_capped(
        &self,
        job: &NewJobPost,
        since: DateTime<Utc>,
        max: u64,
    ) -> Result<Option<i32>, Error>;

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
    /// newest first: `limit` after the first `offset`. Empty when it has no
    /// business, no position or no service.
    async fn open_near_owner(
        &self,
        owner_user: i32,
        since: DateTime<Utc>,
        radius_m: f64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<JobPost>, Error>;

    /// How many posts `open_near_owner` finds, over all its pages.
    async fn count_open_near_owner(
        &self,
        owner_user: i32,
        since: DateTime<Utc>,
        radius_m: f64,
    ) -> Result<i64, Error>;

    /// A page of every post (`state`: only those in that status; `search`:
    /// the farmer's name or phone, or the description), newest first.
    async fn list_admin(
        &self,
        filter: &AdminFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<JobPost>, Error>;

    /// How many posts `list_admin` pages through.
    async fn count_admin(&self, filter: &AdminFilter) -> Result<i64, Error>;

    /// False when there is no such post.
    async fn set_status(&self, id: i32, status: &str) -> Result<bool, Error>;

    /// Deletes the posts made before `before`, whatever their status; how many.
    async fn delete_older_than(&self, before: DateTime<Utc>) -> Result<u64, Error>;
}
