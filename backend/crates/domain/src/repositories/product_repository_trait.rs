use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    error::Error,
    models::{
        business::ListingEditor,
        product::{
            AdminProduct, AdminProductFilter, NewProduct, ProductFilter, ProductSort, ProductView,
            ProviderProduct, UpdateProduct, UpdateProductAdmin,
        },
    },
};

#[async_trait]
pub trait ProductRepositoryTrait {
    /// The admin list, narrowed to one provider and / or to names containing
    /// `search` (accents and case ignored).
    async fn list_admin(
        &self,
        filter: &AdminProductFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AdminProduct>, Error>;

    async fn count_admin(&self, filter: &AdminProductFilter) -> Result<i64, Error>;

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<AdminProduct>, Error>;

    async fn admin_toggle_active(&self, id: i32, is_active: bool) -> Result<bool, Error>;

    async fn admin_delete(&self, id: i32) -> Result<bool, Error>;

    async fn browse(
        &self,
        filter: &ProductFilter,
        sort: ProductSort,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ProductView>, Error>;

    async fn count_browse(&self, filter: &ProductFilter) -> Result<i64, Error>;

    /// The newest shown supplies, those with a photo first.
    async fn featured(&self, limit: i64, sector: Option<&str>) -> Result<Vec<ProductView>, Error>;

    async fn find_by_id(&self, id: i32) -> Result<Option<ProductView>, Error>;

    /// Live products whose seller lies within `radius` metres of
    /// `point_wkt`, nearest first: `limit` after the first `offset`.
    async fn search_geo(
        &self,
        point_wkt: &str,
        radius: f64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ProductView>, Error>;

    /// How many products `search_geo` finds, over all its pages.
    async fn count_geo(&self, point_wkt: &str, radius: f64) -> Result<i64, Error>;

    async fn list_by_user(&self, user_id: i32) -> Result<Vec<ProviderProduct>, Error>;

    /// A shown supply type: what a supply may be filed under.
    async fn check_category_is_product(&self, category_id: i32) -> Result<bool, Error>;

    /// A shown commodity: what a supply may say it is for.
    async fn check_commodity_is_shown(&self, commodity_id: i32) -> Result<bool, Error>;

    /// Creates the supply unless its business already lists `cap` of them
    /// (hidden ones count): `None` then. Counted and inserted in one
    /// transaction with the business locked, so two requests cannot both
    /// take the last place.
    async fn create_capped(&self, product: NewProduct, cap: u64) -> Result<Option<i32>, Error>;

    async fn find_provider_product_by_id(&self, id: i32) -> Result<Option<ProviderProduct>, Error>;

    async fn update(&self, product: UpdateProduct) -> Result<bool, Error>;

    /// The admin's edit; false when there is no such live product.
    async fn update_admin(&self, product: UpdateProductAdmin) -> Result<bool, Error>;

    async fn soft_delete(&self, id: i32, owner_user: i32) -> Result<bool, Error>;

    async fn count_active(&self) -> Result<i64, Error>;

    /// Points a live product at `file_id` (`None`: no image). `None` when
    /// there is no such product `editor` may change; otherwise the image it
    /// had before. The old value is read under a row lock, so a concurrent
    /// upload cannot orphan the image it replaced.
    async fn set_image(
        &self,
        id: i32,
        editor: ListingEditor,
        file_id: Option<Uuid>,
    ) -> Result<Option<Option<Uuid>>, Error>;

    /// The image of a product, deleted or not: cleanup runs after a delete.
    async fn image_of(&self, id: i32) -> Result<Option<Uuid>, Error>;
}
