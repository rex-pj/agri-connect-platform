use async_trait::async_trait;

use crate::{
    error::Error,
    models::business::{
        BusinessAdminDetail, BusinessAdminView, BusinessBrowse, BusinessDetail, BusinessGeoResult,
        BusinessProfile,
    },
};

#[async_trait]
pub trait BusinessRepositoryTrait {
    async fn search_geo(
        &self,
        point_wkt: &str,
        radius: f64,
        service_id: Option<i32>,
    ) -> Result<Vec<BusinessGeoResult>, Error>;

    async fn browse(
        &self,
        service_id: Option<i32>,
        name: Option<String>,
        sort: Option<String>,
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

    async fn check_exists(&self, id: i32) -> Result<bool, Error>;

    async fn list_for_admin(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<BusinessAdminView>, Error>;

    async fn toggle_verify(&self, id: i32) -> Result<Option<(i32, String, bool)>, Error>;

    async fn check_exists_for_admin(&self, id: i32) -> Result<bool, Error>;

    async fn assign_user(&self, business_id: i32, user_id: i32) -> Result<(), Error>;

    async fn get_profile_by_user(&self, user_id: i32) -> Result<Option<BusinessProfile>, Error>;

    async fn update_profile_text(
        &self,
        user_id: i32,
        name: &str,
        phone: &str,
        description: Option<&str>,
        address: Option<&str>,
    ) -> Result<bool, Error>;

    async fn update_location(&self, user_id: i32, point_wkt: &str) -> Result<(), Error>;

    async fn get_business_id_by_user(&self, user_id: i32) -> Result<Option<i32>, Error>;

    async fn find_admin_detail(&self, id: i32) -> Result<Option<BusinessAdminDetail>, Error>;

    async fn count_all(&self) -> Result<i64, Error>;

    async fn count_verified(&self) -> Result<i64, Error>;

    /// Create a new business during provider self-registration.
    /// Returns the new business `id`.
    async fn create_for_registration(
        &self,
        name: &str,
        phone: &str,
        user_id: i32,
    ) -> Result<i32, Error>;
}
