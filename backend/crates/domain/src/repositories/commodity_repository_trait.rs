use async_trait::async_trait;

use crate::{
    error::Error,
    models::commodity::{Commodity, CommodityFields},
};

#[async_trait]
pub trait CommodityRepositoryTrait {
    /// Every commodity (hidden ones too), by sector order, display order,
    /// then name.
    async fn list_all(&self) -> Result<Vec<Commodity>, Error>;

    /// A page of them (hidden ones too), in the same order.
    async fn list_page(&self, limit: i64, offset: i64) -> Result<Vec<Commodity>, Error>;

    /// How many there are, hidden ones too.
    async fn count(&self) -> Result<i64, Error>;

    async fn find(&self, id: i32) -> Result<Option<Commodity>, Error>;

    /// Its id; a name another commodity has is `commodity_name_taken` on
    /// `name`.
    async fn create(&self, fields: &CommodityFields, actor_id: i32) -> Result<i32, Error>;

    /// False when there is no such commodity; a taken name as in `create`.
    async fn update(&self, id: i32, fields: &CommodityFields, actor_id: i32)
        -> Result<bool, Error>;

    /// Shows or hides one: `Some((name, changed))`, or `None` when there is
    /// no such commodity.
    async fn set_active(
        &self,
        id: i32,
        active: bool,
        actor_id: i32,
    ) -> Result<Option<(String, bool)>, Error>;
}
