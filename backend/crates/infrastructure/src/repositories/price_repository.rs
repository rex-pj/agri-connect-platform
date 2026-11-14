//! Crops and their prices. A page of prices is put together from the price
//! rows and the crops and areas they name (a few dozen of each), never one
//! query per row.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::NaiveDate;
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use tracing::error;

use domain::{
    models::price::{Crop, CropFields, CropPrice, NewCropPrice},
    repositories::price_repository_trait::PriceRepositoryTrait,
    Error, ErrorCode,
};

use crate::{
    entities::{
        crop::{self, Entity as CropEntity},
        crop_price::{self, Entity as PriceEntity},
        location,
    },
    repositories::unique_or_internal,
};

pub struct PriceRepository {
    db: Arc<DatabaseConnection>,
}

impl PriceRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    fn fail(what: &'static str) -> impl Fn(DbErr) -> Error {
        move |e| {
            error!("price {what} error: {e}");
            Error::internal("Failed to load or save the price.")
        }
    }

    /// Price rows as the domain has them, with their crop and area; rows
    /// whose crop or area is hidden are left out when `public_only`.
    async fn assemble(
        &self,
        rows: Vec<crop_price::Model>,
        public_only: bool,
    ) -> Result<Vec<CropPrice>, Error> {
        let db = self.db.as_ref();
        let crops: HashMap<i32, crop::Model> = CropEntity::find()
            .filter(crop::Column::Id.is_in(rows.iter().map(|r| r.crop_id)))
            .all(db)
            .await
            .map_err(Self::fail("crops of prices"))?
            .into_iter()
            .map(|c| (c.id, c))
            .collect();
        let areas: HashMap<i32, location::Model> = location::Entity::find()
            .filter(location::Column::Id.is_in(rows.iter().map(|r| r.location_id)))
            .all(db)
            .await
            .map_err(Self::fail("areas of prices"))?
            .into_iter()
            .map(|l| (l.id, l))
            .collect();
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let crop = crops.get(&r.crop_id)?;
                let area = areas.get(&r.location_id)?;
                if public_only && !(crop.is_active && area.is_active) {
                    return None;
                }
                Some(CropPrice {
                    id: r.id,
                    crop_id: r.crop_id,
                    crop_name: crop.name.clone(),
                    unit: crop.unit.clone(),
                    location_id: r.location_id,
                    location_label: area.label.clone(),
                    observed_on: r.observed_on,
                    price_low: r.price_low,
                    price_high: r.price_high,
                    note: r.note,
                })
            })
            .collect())
    }
}

fn to_crop(c: crop::Model) -> Crop {
    Crop {
        id: c.id,
        name: c.name,
        commodity_id: c.commodity_id,
        unit: c.unit,
        display_order: c.display_order,
        is_active: c.is_active,
        is_demo: c.is_demo,
    }
}

#[async_trait]
impl PriceRepositoryTrait for PriceRepository {
    async fn list_crops(&self) -> Result<Vec<Crop>, Error> {
        CropEntity::find()
            .order_by_asc(crop::Column::DisplayOrder)
            .order_by_asc(crop::Column::Name)
            .order_by_asc(crop::Column::Id)
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_crop).collect())
            .map_err(Self::fail("list_crops"))
    }

    async fn find_crop(&self, id: i32) -> Result<Option<Crop>, Error> {
        CropEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map(|c| c.map(to_crop))
            .map_err(Self::fail("find_crop"))
    }

    async fn create_crop(&self, f: &CropFields) -> Result<i32, Error> {
        CropEntity::insert(crop::ActiveModel {
            name: Set(f.name.clone()),
            commodity_id: Set(f.commodity_id),
            unit: Set(f.unit.clone()),
            display_order: Set(f.display_order),
            is_active: Set(true),
            ..Default::default()
        })
        .exec(self.db.as_ref())
        .await
        .map(|r| r.last_insert_id)
        .map_err(|e| unique_or_internal(e, "name", ErrorCode::CropNameTaken, "crop create"))
    }

    async fn update_crop(&self, id: i32, f: &CropFields) -> Result<bool, Error> {
        CropEntity::update_many()
            .col_expr(crop::Column::Name, Expr::value(f.name.clone()))
            .col_expr(crop::Column::CommodityId, Expr::value(f.commodity_id))
            .col_expr(crop::Column::Unit, Expr::value(f.unit.clone()))
            .col_expr(crop::Column::DisplayOrder, Expr::value(f.display_order))
            .filter(crop::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| unique_or_internal(e, "name", ErrorCode::CropNameTaken, "crop update"))
    }

    async fn set_crop_active(
        &self,
        id: i32,
        active: bool,
    ) -> Result<Option<(String, bool)>, Error> {
        let Some(current) = CropEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("set_crop_active"))?
        else {
            return Ok(None);
        };
        if current.is_active == active {
            return Ok(Some((current.name, false)));
        }
        CropEntity::update_many()
            .col_expr(crop::Column::IsActive, Expr::value(active))
            .filter(crop::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(Self::fail("set_crop_active"))?;
        Ok(Some((current.name, true)))
    }

    async fn record_price(&self, p: &NewCropPrice, actor_id: i32) -> Result<i32, Error> {
        PriceEntity::insert(crop_price::ActiveModel {
            crop_id: Set(p.crop_id),
            location_id: Set(p.location_id),
            observed_on: Set(p.observed_on),
            price_low: Set(p.price_low),
            price_high: Set(p.price_high),
            note: Set(p.note.clone()),
            created_by: Set(Some(actor_id)),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([
                crop_price::Column::CropId,
                crop_price::Column::LocationId,
                crop_price::Column::ObservedOn,
            ])
            .update_columns([
                crop_price::Column::PriceLow,
                crop_price::Column::PriceHigh,
                crop_price::Column::Note,
                crop_price::Column::CreatedBy,
            ])
            // An admin's price for a demo row's day is real from then on.
            .value(crop_price::Column::IsDemo, Expr::value(false))
            .value(crop_price::Column::CreatedOn, Expr::current_timestamp())
            .to_owned(),
        )
        .exec(self.db.as_ref())
        .await
        .map(|r| r.last_insert_id)
        .map_err(Self::fail("record_price"))
    }

    async fn delete_price(&self, id: i32) -> Result<bool, Error> {
        PriceEntity::delete_by_id(id)
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(Self::fail("delete_price"))
    }

    async fn list_prices(
        &self,
        crop_id: Option<i32>,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<CropPrice>, i64), Error> {
        let db = self.db.as_ref();
        let of_crop = || {
            let query = PriceEntity::find();
            match crop_id {
                Some(id) => query.filter(crop_price::Column::CropId.eq(id)),
                None => query,
            }
        };
        let total = of_crop()
            .count(db)
            .await
            .map_err(Self::fail("count prices"))?;
        let rows = of_crop()
            .order_by_desc(crop_price::Column::ObservedOn)
            .order_by_desc(crop_price::Column::Id)
            .limit(limit)
            .offset(offset)
            .all(db)
            .await
            .map_err(Self::fail("list_prices"))?;
        let total = i64::try_from(total).unwrap_or(i64::MAX);
        Ok((self.assemble(rows, false).await?, total))
    }

    async fn public_prices_since(&self, since: NaiveDate) -> Result<Vec<CropPrice>, Error> {
        let rows = PriceEntity::find()
            .filter(crop_price::Column::ObservedOn.gte(since))
            .order_by_desc(crop_price::Column::ObservedOn)
            .order_by_desc(crop_price::Column::Id)
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("public_prices_since"))?;
        self.assemble(rows, true).await
    }
}
