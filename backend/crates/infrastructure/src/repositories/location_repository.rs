use async_trait::async_trait;
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Select, Set, TransactionTrait, TryInsertResult,
};
use std::{collections::HashSet, sync::Arc};
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

/// Every preset (hidden ones too), by display order then id.
fn admin_order() -> Select<LocationEntity> {
    LocationEntity::find()
        .order_by_asc(location::Column::DisplayOrder)
        .order_by_asc(location::Column::Id)
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
        admin_order()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_location).collect())
            .map_err(|e| {
                error!("location list_all error: {e}");
                Error::internal("Failed to load locations.")
            })
    }

    async fn list_page(&self, limit: i64, offset: i64) -> Result<Vec<Location>, Error> {
        admin_order()
            .limit(limit as u64)
            .offset(offset as u64)
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(to_location).collect())
            .map_err(|e| {
                error!("location list_page error: {e}");
                Error::internal("Failed to load locations.")
            })
    }

    async fn count(&self) -> Result<i64, Error> {
        LocationEntity::find()
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("location count error: {e}");
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

    async fn create_many(&self, presets: Vec<(String, f64, f64)>) -> Result<Vec<Location>, Error> {
        let failed = |e: DbErr| {
            error!("location create_many error: {e}");
            Error::internal("Failed to create locations.")
        };
        if presets.is_empty() {
            return Ok(Vec::new());
        }
        let txn = self.db.begin().await.map_err(failed)?;
        let last: Option<Option<i32>> = LocationEntity::find()
            .select_only()
            .column_as(location::Column::DisplayOrder.max(), "last")
            .into_tuple()
            .one(&txn)
            .await
            .map_err(failed)?;
        let first = last.flatten().unwrap_or(0) + 1;
        let models = presets
            .into_iter()
            .zip(first..)
            .map(|((label, lat, lng), display_order)| ActiveModel {
                label: Set(label),
                lat: Set(lat),
                lng: Set(lng),
                display_order: Set(display_order),
                ..Default::default()
            });
        let inserted = LocationEntity::insert_many(models)
            .on_conflict(
                OnConflict::column(location::Column::Label)
                    .do_nothing()
                    .to_owned(),
            )
            .do_nothing()
            .exec_with_returning_many(&txn)
            .await
            .map_err(failed)?;
        txn.commit().await.map_err(failed)?;
        let mut added = match inserted {
            TryInsertResult::Inserted(rows) => rows,
            TryInsertResult::Empty | TryInsertResult::Conflicted => Vec::new(),
        };
        added.sort_by_key(|r| r.display_order);
        Ok(added.into_iter().map(to_location).collect())
    }

    async fn existing_labels(&self, labels: &[String]) -> Result<HashSet<String>, Error> {
        if labels.is_empty() {
            return Ok(HashSet::new());
        }
        LocationEntity::find()
            .select_only()
            .column(location::Column::Label)
            .filter(location::Column::Label.is_in(labels.iter().cloned()))
            .into_tuple::<String>()
            .all(self.db.as_ref())
            .await
            .map(|found| found.into_iter().collect())
            .map_err(|e| {
                error!("location existing_labels error: {e}");
                Error::internal("Failed to load locations.")
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
