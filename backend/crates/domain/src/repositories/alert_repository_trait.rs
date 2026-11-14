use async_trait::async_trait;
use chrono::NaiveDate;

use crate::{
    error::Error,
    models::alert::{Alert, AlertFields},
};

#[async_trait]
pub trait AlertRepositoryTrait {
    /// Every alert (switched off ones too), the latest to end first.
    async fn list(&self) -> Result<Vec<Alert>, Error>;

    async fn find(&self, id: i32) -> Result<Option<Alert>, Error>;

    /// Its id.
    async fn create(&self, fields: &AlertFields, by: i32) -> Result<i32, Error>;

    /// False when there is no such alert.
    async fn update(&self, id: i32, fields: &AlertFields, by: i32) -> Result<bool, Error>;

    /// Turns an alert on or off; `Some(changed)`, or `None` when there is no
    /// such alert.
    async fn set_active(&self, id: i32, active: bool, by: i32) -> Result<Option<bool>, Error>;

    /// The alerts on and in force on `today` (both dates included), for
    /// `sector` (its own and those of every sector; `None`: all), the
    /// newest first, at most `limit`.
    async fn live(
        &self,
        today: NaiveDate,
        sector: Option<&str>,
        limit: u64,
    ) -> Result<Vec<Alert>, Error>;
}
