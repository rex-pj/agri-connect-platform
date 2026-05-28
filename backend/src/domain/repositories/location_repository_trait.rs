use async_trait::async_trait;

use crate::{domain::models::location::Location, view_models::AppError};

#[async_trait]
pub trait LocationRepositoryTrait {
    // ── Public ────────────────────────────────────────────────────────────────
    async fn list_active(&self) -> Result<Vec<Location>, AppError>;

    // ── Admin ─────────────────────────────────────────────────────────────────
    async fn list_all(&self) -> Result<Vec<Location>, AppError>;
    async fn find_by_id(&self, id: i32) -> Result<Option<Location>, AppError>;
    async fn create(&self, label: String, lat: f64, lng: f64, display_order: i32) -> Result<i32, AppError>;
    async fn update(&self, id: i32, label: String, lat: f64, lng: f64, display_order: i32) -> Result<(), AppError>;
    async fn toggle_active(&self, id: i32, is_active: bool) -> Result<(), AppError>;
    async fn delete(&self, id: i32) -> Result<(), AppError>;
}
