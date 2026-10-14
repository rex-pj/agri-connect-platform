use async_trait::async_trait;

use crate::{error::Error, models::location::Location};

#[async_trait]
pub trait LocationRepositoryTrait {
    // ── Public ────────────────────────────────────────────────────────────────
    async fn list_active(&self) -> Result<Vec<Location>, Error>;

    // ── Admin ─────────────────────────────────────────────────────────────────
    async fn list_all(&self) -> Result<Vec<Location>, Error>;
    async fn find_by_id(&self, id: i32) -> Result<Option<Location>, Error>;
    async fn create(
        &self,
        label: String,
        lat: f64,
        lng: f64,
        display_order: i32,
    ) -> Result<i32, Error>;
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
