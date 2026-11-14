use async_trait::async_trait;
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use std::sync::Arc;
use tracing::error;

use domain::{
    models::location::Location, repositories::location_repository_trait::LocationRepositoryTrait,
};

use domain::{Error, ErrorCode};

use crate::entities::location::{self, ActiveModel, Entity as LocationEntity};
use crate::repositories::unique_or_internal;

pub struct LocationRepository {
    db: Arc<DatabaseConnection>,
}

impl LocationRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

fn to_location(r: location::Model) -> Location {
    Location {
        id: r.id,
        label: r.label,
        lat: r.lat,
        lng: r.lng,
        display_order: r.display_order,
        is_active: r.is_active,
        is_demo: r.is_demo,
    }
}

#[async_trait]
impl LocationRepositoryTrait for LocationRepository {
    async fn list_active(&self) -> Result<Vec<Location>, Error> {
        LocationEntity::find()
            .filter(location::Column::IsActive.eq(true))
            .order_by_asc(location::Column::DisplayOrder)
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_location).collect())
            .map_err(|e| {
                error!("location list_active error: {e}");
                Error::internal("Failed to load locations.")
            })
    }

    async fn list_all(&self) -> Result<Vec<Location>, Error> {
        LocationEntity::find()
            .order_by_asc(location::Column::DisplayOrder)
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_location).collect())
            .map_err(|e| {
                error!("location list_all error: {e}");
                Error::internal("Failed to load locations.")
            })
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<Location>, Error> {
        LocationEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(to_location))
            .map_err(|e| {
                error!("location find_by_id error: {e}");
                Error::internal("Failed to load location.")
            })
    }

    async fn create(
        &self,
        label: String,
        lat: f64,
        lng: f64,
        display_order: i32,
    ) -> Result<i32, Error> {
        let model = ActiveModel {
            label: Set(label),
            lat: Set(lat),
            lng: Set(lng),
            display_order: Set(display_order),
            ..Default::default()
        };
        LocationEntity::insert(model)
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| {
                unique_or_internal(e, "label", ErrorCode::LocationLabelTaken, "location create")
            })
    }

    async fn update(
        &self,
        id: i32,
        label: String,
        lat: f64,
        lng: f64,
        display_order: i32,
    ) -> Result<(), Error> {
        let result = LocationEntity::update_many()
            .col_expr(location::Column::Label, Expr::value(label))
            .col_expr(location::Column::Lat, Expr::value(lat))
            .col_expr(location::Column::Lng, Expr::value(lng))
            .col_expr(location::Column::DisplayOrder, Expr::value(display_order))
            .filter(location::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                unique_or_internal(e, "label", ErrorCode::LocationLabelTaken, "location update")
            })?;

        if result.rows_affected == 0 {
            return Err(Error::new(ErrorCode::LocationNotFound));
        }
        Ok(())
    }

    async fn toggle_active(&self, id: i32, is_active: bool) -> Result<(), Error> {
        let result = LocationEntity::update_many()
            .col_expr(location::Column::IsActive, Expr::value(is_active))
            .filter(location::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("location toggle_active error: {e}");
                Error::internal("Failed to update location status.")
            })?;

        if result.rows_affected == 0 {
            return Err(Error::new(ErrorCode::LocationNotFound));
        }
        Ok(())
    }

    async fn delete(&self, id: i32) -> Result<(), Error> {
        let result = LocationEntity::delete_by_id(id)
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("location delete error: {e}");
                Error::internal("Failed to delete location.")
            })?;

        if result.rows_affected == 0 {
            return Err(Error::new(ErrorCode::LocationNotFound));
        }
        Ok(())
    }
}
