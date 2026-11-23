use async_trait::async_trait;

use crate::{
    error::Error,
    models::coded::{CodedEntry, CodedFields},
};

/// One list of coded entries (the sectors, the alert kinds), each its own
/// table with the same columns.
#[async_trait]
pub trait CodedRepositoryTrait {
    /// Every entry (hidden ones too), by display order, then name.
    async fn list_all(&self) -> Result<Vec<CodedEntry>, Error>;

    async fn find(&self, id: i32) -> Result<Option<CodedEntry>, Error>;

    /// Its id. A name (or the code made from it) another entry has is the
    /// list's name-taken error on `name`.
    async fn create(&self, code: &str, fields: &CodedFields, actor_id: i32) -> Result<i32, Error>;

    /// False when there is no such entry; a taken name as in `create`.
    async fn update(&self, id: i32, fields: &CodedFields, actor_id: i32) -> Result<bool, Error>;

    /// Shows or hides one: `Some((name, changed))`, or `None` when there is
    /// no such entry.
    async fn set_active(
        &self,
        id: i32,
        active: bool,
        actor_id: i32,
    ) -> Result<Option<(String, bool)>, Error>;
}
