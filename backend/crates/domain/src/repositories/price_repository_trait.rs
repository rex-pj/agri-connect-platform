use async_trait::async_trait;
use chrono::NaiveDate;

use crate::{
    error::Error,
    models::price::{Crop, CropFields, CropPrice, NewCropPrice},
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

    /// Every price, the newest day first; a page of them and the total.
    async fn list_prices(&self, limit: u64, offset: u64) -> Result<(Vec<CropPrice>, i64), Error>;

    /// The prices observed on or after `since`, of shown crops in shown
    /// areas, the newest day first.
    async fn public_prices_since(&self, since: NaiveDate) -> Result<Vec<CropPrice>, Error>;
}
