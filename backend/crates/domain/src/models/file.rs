//! An uploaded image as stored: one main object and one thumbnail, both
//! re-encoded by us, so `content_type` is ours, never the uploader's claim.

use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFile {
    /// Random and never reused: a URL built from it may be cached forever.
    pub id: Uuid,
    pub storage_key: String,
    pub thumb_key: String,
    pub content_type: String,
    pub byte_size: i64,
    pub width: i32,
    pub height: i32,
    /// Hex SHA-256 of the main object.
    pub sha256: String,
    pub uploaded_by: Option<i32>,
    pub created_on: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFile {
    pub id: Uuid,
    pub storage_key: String,
    pub thumb_key: String,
    pub content_type: String,
    pub byte_size: i64,
    pub width: i32,
    pub height: i32,
    pub sha256: String,
    pub uploaded_by: i32,
}

/// Which stored object of a file to serve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileVariant {
    Main,
    Thumb,
}

impl StoredFile {
    pub fn key(&self, variant: FileVariant) -> &str {
        match variant {
            FileVariant::Main => &self.storage_key,
            FileVariant::Thumb => &self.thumb_key,
        }
    }
}
