use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, Set};
use std::sync::Arc;

use domain::{
    models::call::NewCallClick, repositories::call_repository_trait::CallRepositoryTrait, Error,
};

use crate::entities::call_click::{self, Entity as CallClick};

pub struct CallRepository {
    db: Arc<DatabaseConnection>,
}

impl CallRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CallRepositoryTrait for CallRepository {
    async fn record(&self, click: NewCallClick) -> Result<(), Error> {
        CallClick::insert(call_click::ActiveModel {
            business_id: Set(click.business_id),
            service_id: Set(click.service_id),
            product_id: Set(click.product_id),
            channel: Set(click.channel),
            ..Default::default()
        })
        .exec(self.db.as_ref())
        .await
        .map(|_| ())
        .map_err(|e| Error::internal(format!("call record: {e}")))
    }

    async fn count_since(
        &self,
        business_id: Option<i32>,
        since: DateTime<Utc>,
    ) -> Result<i64, Error> {
        let mut query = CallClick::find().filter(call_click::Column::CreatedOn.gte(since));
        if let Some(id) = business_id {
            query = query.filter(call_click::Column::BusinessId.eq(id));
        }
        query
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| Error::internal(format!("call count_since: {e}")))
    }
}
