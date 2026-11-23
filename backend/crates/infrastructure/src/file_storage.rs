//! `FileStorage` over `object_store`: a private Cloudflare R2 or GCS bucket in
//! production, a directory otherwise, memory in tests. Same code path for all.

use std::{path::Path, sync::Arc};

use application::ports::FileStorage;
use async_trait::async_trait;
use domain::{Error, ErrorCode, Result};
use object_store::{
    aws::AmazonS3Builder, gcp::GoogleCloudStorageBuilder, local::LocalFileSystem, memory::InMemory,
    path::Path as Key, ObjectStore, PutPayload,
};

/// A private R2 bucket and an R2 API token's S3 credentials (Object Read &
/// Write on that bucket).
pub struct R2Config {
    pub account_id: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

pub struct ObjectFileStorage {
    store: Arc<dyn ObjectStore>,
}

impl ObjectFileStorage {
    /// R2 through its S3 API. The bucket stays private: images are served by
    /// the app (`/files/{id}`), never from the bucket.
    pub fn r2(config: &R2Config) -> Result<Self> {
        let store = AmazonS3Builder::new()
            .with_endpoint(format!(
                "https://{}.r2.cloudflarestorage.com",
                config.account_id
            ))
            // R2 takes "auto"; the signature needs some region.
            .with_region("auto")
            .with_bucket_name(&config.bucket)
            .with_access_key_id(&config.access_key_id)
            .with_secret_access_key(&config.secret_access_key)
            .build()
            .map_err(|e| Error::internal(format!("R2 bucket {}: {e}", config.bucket)))?;
        Ok(Self {
            store: Arc::new(store),
        })
    }

    /// No key file: with none configured, object_store asks the VM's metadata
    /// server for the attached service account's token.
    pub fn gcs(bucket: &str) -> Result<Self> {
        let store = GoogleCloudStorageBuilder::new()
            .with_bucket_name(bucket)
            .build()
            .map_err(|e| Error::internal(format!("GCS bucket {bucket}: {e}")))?;
        Ok(Self {
            store: Arc::new(store),
        })
    }

    /// Creates `dir` if needed. Keys become paths under it.
    pub fn local(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)
            .map_err(|e| Error::internal(format!("upload dir {}: {e}", dir.display())))?;
        let store = LocalFileSystem::new_with_prefix(dir)
            .map_err(|e| Error::internal(format!("upload dir {}: {e}", dir.display())))?;
        Ok(Self {
            store: Arc::new(store),
        })
    }

    /// Lost on restart; for tests.
    pub fn in_memory() -> Self {
        Self {
            store: Arc::new(InMemory::new()),
        }
    }
}

fn key(raw: &str) -> Result<Key> {
    Key::parse(raw).map_err(|e| Error::internal(format!("storage key {raw:?}: {e}")))
}

#[async_trait]
impl FileStorage for ObjectFileStorage {
    async fn put(&self, raw: &str, bytes: Vec<u8>) -> Result<()> {
        self.store
            .put(&key(raw)?, PutPayload::from(bytes))
            .await
            .map(|_| ())
            .map_err(|e| Error::internal(format!("storage put {raw}: {e}")))
    }

    async fn get(&self, raw: &str) -> Result<Vec<u8>> {
        let result = match self.store.get(&key(raw)?).await {
            Ok(r) => r,
            // The row exists but the object does not: same answer as a missing
            // row for the client, an error worth a look in the log.
            Err(object_store::Error::NotFound { .. }) => {
                return Err(Error::new(ErrorCode::FileNotFound)
                    .with_detail(format!("object {raw} missing for an existing row")))
            }
            Err(e) => return Err(Error::internal(format!("storage get {raw}: {e}"))),
        };
        result
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| Error::internal(format!("storage read {raw}: {e}")))
    }

    async fn delete(&self, raw: &str) -> Result<()> {
        match self.store.delete(&key(raw)?).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(Error::internal(format!("storage delete {raw}: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn round_trip(storage: &ObjectFileStorage) {
        let k = "images/test.jpg";
        assert!(storage.put(k, b"abc".to_vec()).await.is_ok());
        assert_eq!(storage.get(k).await.ok(), Some(b"abc".to_vec()));
        assert!(storage.delete(k).await.is_ok());
        // Cleanup may run twice; the second delete must not fail.
        assert!(storage.delete(k).await.is_ok());
        assert_eq!(
            storage.get(k).await.err().map(|e| e.code()),
            Some(ErrorCode::FileNotFound)
        );
    }

    #[test]
    fn r2_storage_builds_from_its_account_and_token() {
        let config = R2Config {
            account_id: "0123456789abcdef0123456789abcdef".to_owned(),
            bucket: "uploads".to_owned(),
            access_key_id: "key".to_owned(),
            secret_access_key: "secret".to_owned(),
        };
        assert!(ObjectFileStorage::r2(&config).is_ok());
    }

    #[tokio::test]
    async fn memory_storage_round_trips() {
        round_trip(&ObjectFileStorage::in_memory()).await;
    }

    #[tokio::test]
    async fn local_storage_round_trips_and_creates_its_directory() {
        let dir = std::env::temp_dir().join(format!("agri-upload-test-{}", std::process::id()));
        let storage = ObjectFileStorage::local(&dir.join("nested"));
        assert!(storage.is_ok());
        if let Ok(storage) = storage {
            round_trip(&storage).await;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
