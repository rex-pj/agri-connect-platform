use std::collections::HashSet;

use async_trait::async_trait;

use crate::{error::Error, models::location::Location};

#[async_trait]
pub trait LocationRepositoryTrait {
    // ── Public ────────────────────────────────────────────────────────────────
    async fn list_active(&self) -> Result<Vec<Location>, Error>;

    // ── Admin ─────────────────────────────────────────────────────────────────
    async fn list_all(&self) -> Result<Vec<Location>, Error>;
    /// A page of every preset (hidden ones too), by display order then id.
    async fn list_page(&self, limit: i64, offset: i64) -> Result<Vec<Location>, Error>;
    /// How many presets there are, hidden ones too.
    async fn count(&self) -> Result<i64, Error>;
    async fn find_by_id(&self, id: i32) -> Result<Option<Location>, Error>;
    async fn create(
        &self,
        label: String,
        lat: f64,
        lng: f64,
        display_order: i32,
    ) -> Result<i32, Error>;
    /// Adds the presets `(label, lat, lng)` in the order given, after the
    /// last display order, in one statement; one whose label is taken
    /// (already, or by a concurrent write) is passed over. The presets added.
    async fn create_many(&self, presets: Vec<(String, f64, f64)>) -> Result<Vec<Location>, Error>;
    /// Which of `labels` a preset already has.
    async fn existing_labels(&self, labels: &[String]) -> Result<HashSet<String>, Error>;
    async fn update(
        &self,
        id: i32,
        label: String,
        lat: f64,
        lng: f64,
        display_order: i32,
    ) -> Result<(), Error>;
    async fn toggle_active(&self, id: i32, is_active: bool) -> Result<(), Error>;
    async fn delete(&self, id: i32) -> Result<(), Error>;
}
