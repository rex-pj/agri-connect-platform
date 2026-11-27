use async_trait::async_trait;
use chrono::NaiveDate;

use crate::{
    error::Error,
    models::info_page::{InfoPage, InfoPageFields, InfoRevision, NewInfoRevision},
};

#[async_trait]
pub trait InfoPageRepositoryTrait {
    /// Every page (off ones too), by footer group, order and id: a short
    /// list an admin keeps, never paged.
    async fn list_all(&self) -> Result<Vec<InfoPage>, Error>;

    /// The pages whose stage is one of `stages`, in the same order.
    async fn with_stage(&self, stages: &[&str]) -> Result<Vec<InfoPage>, Error>;

    async fn find(&self, id: i32) -> Result<Option<InfoPage>, Error>;

    async fn find_by_code(&self, code: &str) -> Result<Option<InfoPage>, Error>;

    /// Its id; a code another page has is `info_page_code_taken` on `code`.
    async fn create(&self, fields: &InfoPageFields, by: i32) -> Result<i32, Error>;

    /// Every box but the code; false when there is no such page.
    async fn update(&self, id: i32, fields: &InfoPageFields, by: i32) -> Result<bool, Error>;

    /// `Some(changed)`, or `None` when there is no such page.
    async fn set_stage(&self, id: i32, stage: &str, by: i32) -> Result<Option<bool>, Error>;

    /// The new version's id.
    async fn add_revision(
        &self,
        page_id: i32,
        revision: &NewInfoRevision,
        by: i32,
    ) -> Result<i32, Error>;

    /// Every version of a page, the latest to come into force first.
    async fn revisions(&self, page_id: i32) -> Result<Vec<InfoRevision>, Error>;

    /// The pages with a version in force on `today`, in one query.
    async fn with_text(&self, today: NaiveDate) -> Result<Vec<i32>, Error>;
}
