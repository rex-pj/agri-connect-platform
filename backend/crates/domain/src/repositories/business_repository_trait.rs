use async_trait::async_trait;

use crate::{
    error::Error,
    models::business::{
        BusinessAdminDetail, BusinessAdminView, BusinessBrowse, BusinessDetail, BusinessGeoResult,
        BusinessProfile, ProfileUpdate, ProviderSort,
    },
};

#[async_trait]
pub trait BusinessRepositoryTrait {
    async fn search_geo(
        &self,
        point_wkt: &str,
        radius: f64,
        service_id: Option<i32>,
        limit: i64,
    ) -> Result<Vec<BusinessGeoResult>, Error>;

    async fn browse(
        &self,
        service_id: Option<i32>,
        name: Option<String>,
        sort: ProviderSort,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<BusinessBrowse>, Error>;

    async fn count_browse(
        &self,
        service_id: Option<i32>,
        name: Option<String>,
    ) -> Result<i64, Error>;

    async fn featured(&self, limit: i64) -> Result<Vec<BusinessBrowse>, Error>;

    async fn find_detail(&self, id: i32) -> Result<Option<BusinessDetail>, Error>;

    async fn list_for_admin(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<BusinessAdminView>, Error>;

    /// Sets the badge; `Some((name, changed))`, or `None` when there is no
    /// such business.
    async fn set_verified(&self, id: i32, verified: bool) -> Result<Option<(String, bool)>, Error>;

    /// Turns the business on or off (off: gone from every public page);
    /// `Some((name, changed))`, or `None` when there is no such business.
    async fn set_active(
        &self,
        id: i32,
        active: bool,
        actor_id: i32,
    ) -> Result<Option<(String, bool)>, Error>;

    async fn check_exists_for_admin(&self, id: i32) -> Result<bool, Error>;

    async fn assign_user(&self, business_id: i32, user_id: i32) -> Result<(), Error>;

    async fn get_profile_by_user(&self, user_id: i32) -> Result<Option<BusinessProfile>, Error>;

    /// Replaces the profile of the business `user_id` owns, location included
    /// (`None` removes it), in one statement. False when they own none.
    async fn update_profile(&self, user_id: i32, profile: &ProfileUpdate) -> Result<bool, Error>;

    async fn get_business_id_by_user(&self, user_id: i32) -> Result<Option<i32>, Error>;

    async fn find_admin_detail(&self, id: i32) -> Result<Option<BusinessAdminDetail>, Error>;

    async fn count_all(&self) -> Result<i64, Error>;

    async fn count_verified(&self) -> Result<i64, Error>;
}
