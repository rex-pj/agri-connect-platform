//! Commodities (cây trồng, vật nuôi): what farmers raise, each in one sector.

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use tracing::error;

use domain::{
    models::commodity::{Commodity, CommodityFields},
    repositories::commodity_repository_trait::CommodityRepositoryTrait,
    Error, ErrorCode,
};

use crate::{
    entities::{commodity, sector},
    repositories::{coded_repository::all_sectors, unique_or_internal},
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
        is_active: c.is_active,
        is_demo: c.is_demo,
    }
}

/// Where a sector comes in the admin's order of the sectors.
fn sector_rank(sectors: &[sector::Model], code: &str) -> usize {
    sectors
        .iter()
        .position(|s| s.code == code)
        .unwrap_or(sectors.len())
}

#[async_trait]
impl CommodityRepositoryTrait for CommodityRepository {
    async fn list_all(&self) -> Result<Vec<Commodity>, Error> {
        let mut all: Vec<Commodity> = commodity::Entity::find()
            .order_by_asc(commodity::Column::DisplayOrder)
            .order_by_asc(commodity::Column::Name)
            .order_by_asc(commodity::Column::Id)
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("list"))?
            .into_iter()
            .map(to_commodity)
            .collect();
        // Stable: within a sector the database's order stays.
        let sectors = all_sectors(self.db.as_ref())
            .await
            .map_err(Self::fail("list"))?;
        all.sort_by_key(|c| sector_rank(&sectors, &c.sector));
        Ok(all)
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
