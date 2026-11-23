//! Crops and their prices. A page of prices is put together from the price
//! rows and the crops and areas they name (a few dozen of each), never one
//! query per row.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::NaiveDate;
use sea_orm::{
    sea_query::{Expr, Func, OnConflict, Order, OverStatement, Query, WindowStatement},
    ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Select, Set,
};
use tracing::error;

use domain::{
    models::price::{Crop, CropFields, CropPrice, NewCropPrice, PriceFilter},
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
    search::{text_contains, PRICE_NOTE_SEARCH_COLUMNS},
    sql::{output, DbFn, QueryAs, TableAlias},
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

/// Every price, or one crop's: what the admin's list and its count read.
fn prices_of(filter: &PriceFilter) -> Select<PriceEntity> {
    let mut query = PriceEntity::find();
    if let Some(id) = filter.location_id {
        query = query.filter(crop_price::Column::LocationId.eq(id));
    }
    if let Some(id) = filter.crop_id {
        query = query.filter(crop_price::Column::CropId.eq(id));
    }
    if let Some(words) = filter.words.as_ref() {
        query = query.filter(
            Condition::any()
                .add(crop_price::Column::CropId.is_in(words.crop_ids.iter().copied()))
                .add(crop_price::Column::LocationId.is_in(words.location_ids.iter().copied()))
                .add(text_contains(PRICE_NOTE_SEARCH_COLUMNS, &words.text)),
        );
    }
    if filter.stale {
        let mut any = Condition::any();
        for (ids, cutoff) in &filter.stale_groups {
            any = any.add(
                Condition::all()
                    .add(crop_price::Column::CropId.is_in(ids.iter().copied()))
                    .add(crop_price::Column::ObservedOn.lt(*cutoff)),
            );
        }
        query = if filter.stale_groups.is_empty() {
            query.filter(crop_price::Column::CropId.is_in(Vec::<i32>::new()))
        } else {
            query.filter(any)
        };
    }
    if let Some(day) = filter.observed_from {
        query = query.filter(crop_price::Column::ObservedOn.gte(day));
    }
    if let Some(day) = filter.observed_to {
        query = query.filter(crop_price::Column::ObservedOn.lte(day));
    }
    query
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
        filter: &PriceFilter,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<CropPrice>, Error> {
        let rows = prices_of(filter)
            .order_by_desc(crop_price::Column::ObservedOn)
            .order_by_desc(crop_price::Column::Id)
            .limit(limit)
            .offset(offset)
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("list_prices"))?;
        self.assemble(rows, false).await
    }

    async fn count_prices(&self, filter: &PriceFilter) -> Result<i64, Error> {
        prices_of(filter)
            .count(self.db.as_ref())
            .await
            .map(|total| i64::try_from(total).unwrap_or(i64::MAX))
            .map_err(Self::fail("count prices"))
    }

    async fn latest_public_prices(
        &self,
        since: NaiveDate,
        per_area: u64,
    ) -> Result<Vec<CropPrice>, Error> {
        // Each crop's prices in each area numbered newest first; the board
        // reads the first few of each, never the whole window.
        let numbered = Query::select()
            .column(crop_price::Column::Id)
            .expr_window_as(
                Func::cust(DbFn::RowNumber),
                WindowStatement::partition_by(crop_price::Column::CropId)
                    .add_partition_by(Expr::col(crop_price::Column::LocationId).into())
                    .order_by(crop_price::Column::ObservedOn, Order::Desc)
                    .order_by(crop_price::Column::Id, Order::Desc)
                    .to_owned(),
                QueryAs::Turn,
            )
            .from(PriceEntity)
            .and_where(crop_price::Column::ObservedOn.gte(since))
            .to_owned();
        let newest = Query::select()
            .column(crop_price::Column::Id)
            .from_subquery(numbered, TableAlias::Ranked)
            .and_where(Expr::expr(output(QueryAs::Turn)).lte(per_area))
            .to_owned();
        let rows = PriceEntity::find()
            .filter(crop_price::Column::Id.in_subquery(newest))
            .order_by_desc(crop_price::Column::ObservedOn)
            .order_by_desc(crop_price::Column::Id)
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("latest_public_prices"))?;
        self.assemble(rows, true).await
    }
}
