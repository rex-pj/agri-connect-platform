use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    error::Error,
    models::file::{NewFile, StoredFile},
};

/// The `files` rows. Objects in storage are the `FileStorage` port's business;
/// this only records what was stored where.
#[async_trait]
pub trait FileRepositoryTrait {
    async fn insert(&self, file: &NewFile) -> Result<(), Error>;

    async fn find(&self, id: Uuid) -> Result<Option<StoredFile>, Error>;

    /// Deletes the row and returns it, so the caller can remove its objects.
    /// References to it (`products.image_file_id`) are cleared by the foreign
    /// key. `None` when it was already gone.
    async fn delete(&self, id: Uuid) -> Result<Option<StoredFile>, Error>;
}
