use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use domain::{
    models::file::{NewFile, StoredFile},
    repositories::file_repository_trait::FileRepositoryTrait,
    Error,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::entities::file::{self, Entity as File};

pub struct FileRepository {
    db: Arc<DatabaseConnection>,
}

impl FileRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

fn to_domain(m: file::Model) -> StoredFile {
    StoredFile {
        id: m.id,
        storage_key: m.storage_key,
        thumb_key: m.thumb_key,
        content_type: m.content_type,
        byte_size: m.byte_size,
        width: m.width,
        height: m.height,
        sha256: m.sha256,
        uploaded_by: m.uploaded_by,
        created_on: m.created_on.with_timezone(&Utc),
    }
}

#[async_trait]
impl FileRepositoryTrait for FileRepository {
    async fn insert(&self, f: &NewFile) -> Result<(), Error> {
        let row = file::ActiveModel {
            id: Set(f.id),
            storage_key: Set(f.storage_key.clone()),
            thumb_key: Set(f.thumb_key.clone()),
            content_type: Set(f.content_type.clone()),
            byte_size: Set(f.byte_size),
            width: Set(f.width),
            height: Set(f.height),
            sha256: Set(f.sha256.clone()),
            uploaded_by: Set(Some(f.uploaded_by)),
            ..Default::default()
        };
        File::insert(row)
            .exec_without_returning(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| Error::internal(format!("file insert ({}): {e}", f.id)))
    }

    async fn find(&self, id: Uuid) -> Result<Option<StoredFile>, Error> {
        File::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map(|m| m.map(to_domain))
            .map_err(|e| Error::internal(format!("file find ({id}): {e}")))
    }

    async fn delete(&self, id: Uuid) -> Result<Option<StoredFile>, Error> {
        // DELETE ... RETURNING: of two cleanups racing on the same file, exactly
        // one gets the row back and removes the objects.
        File::delete_many()
            .filter(file::Column::Id.eq(id))
            .exec_with_returning(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().next().map(to_domain))
            .map_err(|e| Error::internal(format!("file delete ({id}): {e}")))
    }
}
