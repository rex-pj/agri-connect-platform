//! Commodities (cây trồng, vật nuôi): what farmers raise, each in one sector.

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Select, Set,
};
use tracing::error;

use domain::{
    models::commodity::{Commodity, CommodityFields},
    repositories::commodity_repository_trait::CommodityRepositoryTrait,
    Error, ErrorCode,
};

use crate::{
    entities::{commodity, sector},
    repositories::unique_or_internal,
};

pub struct CommodityRepository {
    db: Arc<DatabaseConnection>,
}

impl CommodityRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    fn fail(what: &'static str) -> impl Fn(DbErr) -> Error {
        move |e| {
            error!("commodity {what} error: {e}");
            Error::internal("Failed to load or save the commodity.")
        }
    }
}

fn to_commodity(c: commodity::Model) -> Commodity {
    Commodity {
        id: c.id,
        name: c.name,
        sector: c.sector,
        icon: c.icon,
        display_order: c.display_order,
        price_fresh_days: c.price_fresh_days,
        is_active: c.is_active,
        is_demo: c.is_demo,
    }
}

/// Every commodity in the admin's order: by its sector's place among the
/// sectors, then its own display order, name and id.
fn ordered() -> Select<commodity::Entity> {
    commodity::Entity::find()
        .join(
            JoinType::InnerJoin,
            commodity::Entity::belongs_to(sector::Entity)
                .from(commodity::Column::Sector)
                .to(sector::Column::Code)
                .into(),
        )
        .order_by_asc(sector::Column::DisplayOrder)
        .order_by_asc(sector::Column::Name)
        .order_by_asc(sector::Column::Id)
        .order_by_asc(commodity::Column::DisplayOrder)
        .order_by_asc(commodity::Column::Name)
        .order_by_asc(commodity::Column::Id)
}

#[async_trait]
impl CommodityRepositoryTrait for CommodityRepository {
    async fn list_all(&self) -> Result<Vec<Commodity>, Error> {
        ordered()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_commodity).collect())
            .map_err(Self::fail("list"))
    }

    async fn list_page(&self, limit: i64, offset: i64) -> Result<Vec<Commodity>, Error> {
        ordered()
            .limit(limit as u64)
            .offset(offset as u64)
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_commodity).collect())
            .map_err(Self::fail("list_page"))
    }

    async fn count(&self) -> Result<i64, Error> {
        commodity::Entity::find()
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(Self::fail("count"))
    }

    async fn find(&self, id: i32) -> Result<Option<Commodity>, Error> {
        commodity::Entity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map(|c| c.map(to_commodity))
            .map_err(Self::fail("find"))
    }

    async fn create(&self, f: &CommodityFields, actor_id: i32) -> Result<i32, Error> {
        commodity::Entity::insert(commodity::ActiveModel {
            name: Set(f.name.clone()),
            sector: Set(f.sector.clone()),
            icon: Set(f.icon.clone()),
            display_order: Set(f.display_order),
            price_fresh_days: Set(f.price_fresh_days),
            created_by: Set(Some(actor_id)),
            ..Default::default()
        })
        .exec(self.db.as_ref())
        .await
        .map(|r| r.last_insert_id)
        .map_err(|e| {
            unique_or_internal(e, "name", ErrorCode::CommodityNameTaken, "commodity create")
        })
    }

    async fn update(&self, id: i32, f: &CommodityFields, actor_id: i32) -> Result<bool, Error> {
        commodity::Entity::update_many()
            .col_expr(commodity::Column::Name, Expr::value(f.name.clone()))
            .col_expr(commodity::Column::Sector, Expr::value(f.sector.clone()))
            .col_expr(commodity::Column::Icon, Expr::value(f.icon.clone()))
            .col_expr(
                commodity::Column::DisplayOrder,
                Expr::value(f.display_order),
            )
            .col_expr(
                commodity::Column::PriceFreshDays,
                Expr::value(f.price_fresh_days),
            )
            .col_expr(commodity::Column::UpdatedBy, Expr::value(Some(actor_id)))
            .filter(commodity::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                unique_or_internal(e, "name", ErrorCode::CommodityNameTaken, "commodity update")
            })
    }

    async fn set_active(
        &self,
        id: i32,
        active: bool,
        actor_id: i32,
    ) -> Result<Option<(String, bool)>, Error> {
        let Some(current) = commodity::Entity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("set_active"))?
        else {
            return Ok(None);
        };
        if current.is_active == active {
            return Ok(Some((current.name, false)));
        }
        commodity::Entity::update_many()
            .col_expr(commodity::Column::IsActive, Expr::value(active))
            .col_expr(commodity::Column::UpdatedBy, Expr::value(Some(actor_id)))
            .filter(commodity::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(Self::fail("set_active"))?;
        Ok(Some((current.name, true)))
    }
}
