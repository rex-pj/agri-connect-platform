use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    error::Error,
    models::{
        business::ProviderSort,
        service::{
            AdminService, NewService, ProviderService, PublicService, PublicServiceFilter,
            UpdateOwnService, UpdateService,
        },
    },
};

#[async_trait]
pub trait ServiceRepositoryTrait {
    /// The admin list, narrowed to one provider and / or to names containing
    /// `search` (accents and case ignored).
    async fn list_admin(
        &self,
        provider_id: Option<i32>,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AdminService>, Error>;

    async fn count_admin(&self, provider_id: Option<i32>, search: Option<&str>) -> Result<i64, Error>;

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<AdminService>, Error>;

    async fn create(&self, service: NewService) -> Result<i32, Error>;

    /// Creates the service unless its business already has `cap` services
    /// (hidden ones count). The business row is locked while counting, so two
    /// forms sent at once cannot both slip under the cap. `None` when full.
    async fn create_capped(&self, service: NewService, cap: u64) -> Result<Option<i32>, Error>;

    async fn update(&self, service: UpdateService) -> Result<bool, Error>;

    async fn toggle_active(&self, id: i32, is_active: bool) -> Result<bool, Error>;

    async fn delete(&self, id: i32) -> Result<bool, Error>;

    /// The name of an active service category; `None` for anything else (a
    /// product category, a hidden one, a missing id).
    async fn active_service_category_name(&self, category_id: i32)
        -> Result<Option<String>, Error>;

    // ── Owner-scoped: every query matches only services of a live business
    // the user owns, in the same statement, so someone else's service looks
    // the same as a missing one.

    async fn list_by_owner(&self, owner_user: i32) -> Result<Vec<ProviderService>, Error>;

    async fn find_owned(&self, id: i32, owner_user: i32) -> Result<Option<ProviderService>, Error>;

    /// Never touches `is_active`. `false` when the service is not the user's.
    async fn update_owned(&self, service: UpdateOwnService) -> Result<bool, Error>;

    async fn soft_delete_owned(&self, id: i32, owner_user: i32) -> Result<bool, Error>;

    /// Sets the image of one of the user's services; `Some(previous)` when it
    /// was theirs, `None` when not. The old value is read under a row lock, so
    /// a concurrent upload cannot orphan the image it replaced.
    async fn set_image_owned(
        &self,
        id: i32,
        owner_user: i32,
        file_id: Uuid,
    ) -> Result<Option<Option<Uuid>>, Error>;

    /// The image a (possibly soft-deleted) service names.
    async fn image_of(&self, id: i32) -> Result<Option<Uuid>, Error>;

    // ── Public: only live services of live businesses in shown categories.

    /// One page of the listing. Sorted by `sort`, ending with the id so pages
    /// never repeat or skip a row.
    async fn browse_public(
        &self,
        filter: &PublicServiceFilter,
        sort: ProviderSort,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PublicService>, Error>;

    /// Rows `browse_public` would page through, under the same conditions.
    async fn count_public(&self, filter: &PublicServiceFilter) -> Result<i64, Error>;

    async fn find_public(&self, id: i32) -> Result<Option<PublicService>, Error>;

    /// Candidates for "featured": public services of verified providers,
    /// those with a photo first, then the newest, at most `limit`. The caller
    /// keeps one per provider.
    async fn featured_candidates(&self, limit: i64) -> Result<Vec<PublicService>, Error>;

    /// Services whose business lies within `radius` metres of `point_wkt`,
    /// nearest first, at most `limit`.
    async fn search_geo_public(
        &self,
        point_wkt: &str,
        radius: f64,
        category_id: Option<i32>,
        limit: i64,
    ) -> Result<Vec<PublicService>, Error>;
}
