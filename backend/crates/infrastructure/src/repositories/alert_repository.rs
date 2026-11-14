//! Alerts. A kind and a province are named from their tables (a few rows,
//! read once per call), never one query per alert.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use sea_orm::{
    sea_query::Expr, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use tracing::error;

use domain::{
    models::alert::{Alert, AlertFields},
    repositories::alert_repository_trait::AlertRepositoryTrait,
    Error,
};

use crate::entities::{
    alert::{self, Entity as AlertEntity},
    alert_kind,
    province::{self, Entity as ProvinceEntity},
};

pub struct AlertRepository {
    db: Arc<DatabaseConnection>,
}

impl AlertRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    fn fail(what: &'static str) -> impl Fn(DbErr) -> Error {
        move |e| {
            error!("alert {what} error: {e}");
            Error::internal("Failed to load alerts.")
        }
    }

    /// The alerts as the domain has them, each kind and province named.
    async fn named(&self, rows: Vec<alert::Model>) -> Result<Vec<Alert>, Error> {
        let kinds: HashMap<String, alert_kind::Model> = alert_kind::Entity::find()
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("kinds"))?
            .into_iter()
            .map(|k| (k.code.clone(), k))
            .collect();
        let names: HashMap<String, String> = if rows.iter().any(|r| r.province_code.is_some()) {
            ProvinceEntity::find()
                .all(self.db.as_ref())
                .await
                .map_err(Self::fail("provinces"))?
                .into_iter()
                .map(|p: province::Model| (p.code, p.name))
                .collect()
        } else {
            HashMap::new()
        };
        Ok(rows
            .into_iter()
            .map(|r| Alert {
                kind_name: kinds
                    .get(&r.kind)
                    .map_or_else(|| r.kind.clone(), |k| k.name.clone()),
                kind_icon: kinds.get(&r.kind).and_then(|k| k.icon.clone()),
                province_name: r.province_code.as_ref().and_then(|c| names.get(c).cloned()),
                id: r.id,
                kind: r.kind,
                sector: r.sector,
                province_code: r.province_code,
                title: r.title,
                body: r.body,
                starts_on: r.starts_on,
                ends_on: r.ends_on,
                is_active: r.is_active,
                is_demo: r.is_demo,
                updated_on: r.updated_on.with_timezone(&Utc),
            })
            .collect())
    }
}

#[async_trait]
impl AlertRepositoryTrait for AlertRepository {
    async fn list(&self) -> Result<Vec<Alert>, Error> {
        let rows = AlertEntity::find()
            .order_by_desc(alert::Column::EndsOn)
            .order_by_desc(alert::Column::Id)
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("list"))?;
        self.named(rows).await
    }

    async fn find(&self, id: i32) -> Result<Option<Alert>, Error> {
        let row = AlertEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("find"))?;
        Ok(self.named(row.into_iter().collect()).await?.pop())
    }

    async fn create(&self, f: &AlertFields, by: i32) -> Result<i32, Error> {
        AlertEntity::insert(alert::ActiveModel {
            kind: Set(f.kind.clone()),
            sector: Set(f.sector.clone()),
            province_code: Set(f.province_code.clone()),
            title: Set(f.title.clone()),
            body: Set(f.body.clone()),
            starts_on: Set(f.starts_on),
            ends_on: Set(f.ends_on),
            created_by: Set(Some(by)),
            updated_by: Set(Some(by)),
            ..Default::default()
        })
        .exec(self.db.as_ref())
        .await
        .map(|r| r.last_insert_id)
        .map_err(Self::fail("create"))
    }

    async fn update(&self, id: i32, f: &AlertFields, by: i32) -> Result<bool, Error> {
        let result = AlertEntity::update_many()
            .col_expr(alert::Column::Kind, Expr::value(f.kind.clone()))
            .col_expr(alert::Column::Sector, Expr::value(f.sector.clone()))
            .col_expr(
                alert::Column::ProvinceCode,
                Expr::value(f.province_code.clone()),
            )
            .col_expr(alert::Column::Title, Expr::value(f.title.clone()))
            .col_expr(alert::Column::Body, Expr::value(f.body.clone()))
            .col_expr(alert::Column::StartsOn, Expr::value(f.starts_on))
            .col_expr(alert::Column::EndsOn, Expr::value(f.ends_on))
            .col_expr(alert::Column::UpdatedBy, Expr::value(Some(by)))
            .col_expr(alert::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(alert::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(Self::fail("update"))?;
        Ok(result.rows_affected > 0)
    }

    async fn set_active(&self, id: i32, active: bool, by: i32) -> Result<Option<bool>, Error> {
        let Some(current) = AlertEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("set_active"))?
        else {
            return Ok(None);
        };
        if current.is_active == active {
            return Ok(Some(false));
        }
        AlertEntity::update_many()
            .col_expr(alert::Column::IsActive, Expr::value(active))
            .col_expr(alert::Column::UpdatedBy, Expr::value(Some(by)))
            .col_expr(alert::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(alert::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(Self::fail("set_active"))?;
        Ok(Some(true))
    }

    async fn live(
        &self,
        today: NaiveDate,
        sector: Option<&str>,
        limit: u64,
    ) -> Result<Vec<Alert>, Error> {
        let mut query = AlertEntity::find()
            .filter(alert::Column::IsActive.eq(true))
            .filter(alert::Column::StartsOn.lte(today))
            .filter(alert::Column::EndsOn.gte(today));
        if let Some(sector) = sector {
            query = query.filter(
                Condition::any()
                    .add(alert::Column::Sector.eq(sector))
                    .add(alert::Column::Sector.is_null()),
            );
        }
        let rows = query
            .order_by_desc(alert::Column::StartsOn)
            .order_by_desc(alert::Column::Id)
            .limit(limit)
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("live"))?;
        self.named(rows).await
    }
}
