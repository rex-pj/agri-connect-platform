//! The settings moved off their defaults (`platform_settings`).

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{
    sea_query::OnConflict, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use tracing::error;

use domain::{
    models::setting::{SettingWrite, StoredSetting},
    repositories::setting_repository_trait::SettingRepositoryTrait,
    Error,
};

use crate::entities::platform_setting;

pub struct SettingRepository {
    db: Arc<DatabaseConnection>,
}

impl SettingRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    fn fail(what: &'static str) -> impl Fn(DbErr) -> Error {
        move |e| {
            error!("setting {what} error: {e}");
            Error::internal("Failed to load or save the settings.")
        }
    }
}

#[async_trait]
impl SettingRepositoryTrait for SettingRepository {
    async fn all(&self) -> Result<Vec<StoredSetting>, Error> {
        Ok(platform_setting::Entity::find()
            .order_by_asc(platform_setting::Column::Key)
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("list"))?
            .into_iter()
            .map(|s| StoredSetting {
                key: s.key,
                value: s.value,
                updated_on: s.updated_on.to_utc(),
            })
            .collect())
    }

    async fn save(&self, writes: &[SettingWrite], actor_id: i32) -> Result<(), Error> {
        let txn = self.db.begin().await.map_err(Self::fail("begin"))?;
        for write in writes {
            match &write.value {
                Some(value) => {
                    platform_setting::Entity::insert(platform_setting::ActiveModel {
                        key: Set(write.key.clone()),
                        value: Set(value.clone()),
                        updated_by: Set(Some(actor_id)),
                        ..Default::default()
                    })
                    .on_conflict(
                        OnConflict::column(platform_setting::Column::Key)
                            .update_columns([
                                platform_setting::Column::Value,
                                platform_setting::Column::UpdatedBy,
                            ])
                            .to_owned(),
                    )
                    .exec(&txn)
                    .await
                    .map_err(Self::fail("store"))?;
                }
                None => {
                    platform_setting::Entity::delete_many()
                        .filter(platform_setting::Column::Key.eq(write.key.as_str()))
                        .exec(&txn)
                        .await
                        .map_err(Self::fail("reset"))?;
                }
            }
        }
        txn.commit().await.map_err(Self::fail("commit"))
    }
}
