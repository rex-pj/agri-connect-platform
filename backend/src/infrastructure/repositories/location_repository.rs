use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    sea_query::Expr,
};
use std::sync::Arc;
use tracing::error;

use crate::{
    domain::{
        models::location::Location,
        repositories::location_repository_trait::LocationRepositoryTrait,
    },
    entities::location::{self, ActiveModel, Entity as LocationEntity},
    view_models::AppError,
};

pub struct LocationRepository {
    db: Arc<DatabaseConnection>,
}

impl LocationRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

fn to_location(r: location::Model) -> Location {
    Location { id: r.id, label: r.label, lat: r.lat, lng: r.lng, display_order: r.display_order, is_active: r.is_active }
}

#[async_trait]
impl LocationRepositoryTrait for LocationRepository {
    async fn list_active(&self) -> Result<Vec<Location>, AppError> {
        LocationEntity::find()
            .filter(location::Column::IsActive.eq(true))
            .order_by_asc(location::Column::DisplayOrder)
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_location).collect())
            .map_err(|e| {
                error!("location list_active error: {e}");
                AppError::Internal("Failed to load locations.".into())
            })
    }

    async fn list_all(&self) -> Result<Vec<Location>, AppError> {
        LocationEntity::find()
            .order_by_asc(location::Column::DisplayOrder)
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_location).collect())
            .map_err(|e| {
                error!("location list_all error: {e}");
                AppError::Internal("Failed to load locations.".into())
            })
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<Location>, AppError> {
        LocationEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(to_location))
            .map_err(|e| {
                error!("location find_by_id error: {e}");
                AppError::Internal("Failed to load location.".into())
            })
    }

    async fn create(&self, label: String, lat: f64, lng: f64, display_order: i32) -> Result<i32, AppError> {
        let model = ActiveModel {
            label:         Set(label),
            lat:           Set(lat),
            lng:           Set(lng),
            display_order: Set(display_order),
            ..Default::default()
        };
        LocationEntity::insert(model)
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| {
                error!("location create error: {e}");
                AppError::Internal("Failed to create location.".into())
            })
    }

    async fn update(&self, id: i32, label: String, lat: f64, lng: f64, display_order: i32) -> Result<(), AppError> {
        let result = LocationEntity::update_many()
            .col_expr(location::Column::Label,        Expr::value(label))
            .col_expr(location::Column::Lat,          Expr::value(lat))
            .col_expr(location::Column::Lng,          Expr::value(lng))
            .col_expr(location::Column::DisplayOrder, Expr::value(display_order))
            .filter(location::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("location update error: {e}");
                AppError::Internal("Failed to update location.".into())
            })?;

        if result.rows_affected == 0 {
            return Err(AppError::NotFound("Location not found.".into()));
        }
        Ok(())
    }

    async fn toggle_active(&self, id: i32, is_active: bool) -> Result<(), AppError> {
        let result = LocationEntity::update_many()
            .col_expr(location::Column::IsActive, Expr::value(is_active))
            .filter(location::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("location toggle_active error: {e}");
                AppError::Internal("Failed to update location status.".into())
            })?;

        if result.rows_affected == 0 {
            return Err(AppError::NotFound("Location not found.".into()));
        }
        Ok(())
    }

    async fn delete(&self, id: i32) -> Result<(), AppError> {
        let result = LocationEntity::delete_by_id(id)
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("location delete error: {e}");
                AppError::Internal("Failed to delete location.".into())
            })?;

        if result.rows_affected == 0 {
            return Err(AppError::NotFound("Location not found.".into()));
        }
        Ok(())
    }
}
