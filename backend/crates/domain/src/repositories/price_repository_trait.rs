use async_trait::async_trait;
use chrono::NaiveDate;

use crate::{
    error::Error,
    models::price::{Crop, CropFields, CropPrice, NewCropPrice, PriceFilter},
};

#[async_trait]
pub trait PriceRepositoryTrait {
    /// Every crop (hidden ones too), by display order, then name.
    async fn list_crops(&self) -> Result<Vec<Crop>, Error>;

    async fn find_crop(&self, id: i32) -> Result<Option<Crop>, Error>;

    /// Its id; a name another crop has is `crop_name_taken` on `name`.
    async fn create_crop(&self, fields: &CropFields) -> Result<i32, Error>;

    /// False when there is no such crop; a taken name as in `create_crop`.
    async fn update_crop(&self, id: i32, fields: &CropFields) -> Result<bool, Error>;

    /// Shows or hides a crop; `Some((name, changed))`, or `None` when there
    /// is no such crop. A hidden crop's prices are not public.
    async fn set_crop_active(&self, id: i32, active: bool)
        -> Result<Option<(String, bool)>, Error>;

    /// Stores a price, replacing the one of the same crop, area and day;
    /// its id.
    async fn record_price(&self, price: &NewCropPrice, actor_id: i32) -> Result<i32, Error>;

    /// False when there is no such price.
    async fn delete_price(&self, id: i32) -> Result<bool, Error>;

    /// A page of every price (or of one crop's, or one area's), the newest day first.
    async fn list_prices(
        &self,
        filter: &PriceFilter,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<CropPrice>, Error>;

    /// How many prices `list_prices` pages through.
    async fn count_prices(&self, filter: &PriceFilter) -> Result<i64, Error>;

    /// The newest `per_area` prices of each crop in each area, observed on
    /// or after `since`, of shown crops in shown areas, the newest day first.
    async fn latest_public_prices(
        &self,
        since: NaiveDate,
        per_area: u64,
    ) -> Result<Vec<CropPrice>, Error>;
}
