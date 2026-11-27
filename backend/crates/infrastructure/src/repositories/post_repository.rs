//! Articles (news and guides). A province and a commodity are named from
//! their tables, a few rows read once per call, never one query per post.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Select, Set,
};
use tracing::error;
use uuid::Uuid;

use domain::{
    models::{
        admin_list::AdminFilter,
        post::{Post, PostFields, PostQuery},
    },
    repositories::post_repository_trait::PostRepositoryTrait,
    Error,
};

use crate::{
    admin_list::{admin_order, OrderColumns},
    entities::{
        commodity,
        post::{self, Entity as PostEntity},
        province,
    },
    search::admin_post_search,
};

/// The stored status of a post everyone may read.
const PUBLISHED: &str = "published";

pub struct PostRepository {
    db: Arc<DatabaseConnection>,
}

impl PostRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    fn fail(what: &'static str) -> impl Fn(DbErr) -> Error {
        move |e| {
            error!("post {what} error: {e}");
            Error::internal("Failed to load articles.")
        }
    }

    /// The posts as the domain has them, each province and commodity named.
    async fn named(&self, rows: Vec<post::Model>) -> Result<Vec<Post>, Error> {
        let provinces: HashMap<String, String> = if rows.iter().any(|r| r.province_code.is_some()) {
            province::Entity::find()
                .all(self.db.as_ref())
                .await
                .map_err(Self::fail("provinces"))?
                .into_iter()
                .map(|p| (p.code, p.name))
                .collect()
        } else {
            HashMap::new()
        };
        let ids: Vec<i32> = rows.iter().filter_map(|r| r.commodity_id).collect();
        let commodities: HashMap<i32, String> = if ids.is_empty() {
            HashMap::new()
        } else {
            commodity::Entity::find()
                .filter(commodity::Column::Id.is_in(ids))
                .all(self.db.as_ref())
                .await
                .map_err(Self::fail("commodities"))?
                .into_iter()
                .map(|c| (c.id, c.name))
                .collect()
        };
        Ok(rows
            .into_iter()
            .map(|r| Post {
                province_name: r
                    .province_code
                    .as_ref()
                    .and_then(|c| provinces.get(c).cloned()),
                commodity_name: r.commodity_id.and_then(|c| commodities.get(&c).cloned()),
                id: r.id,
                kind: r.kind,
                title: r.title,
                summary: r.summary,
                body: r.body,
                cover_file_id: r.cover_file_id,
                sector: r.sector,
                province_code: r.province_code,
                commodity_id: r.commodity_id,
                status: r.status,
                published_on: r.published_on.map(|d| d.with_timezone(&Utc)),
                is_pinned: r.is_pinned,
                is_demo: r.is_demo,
                updated_on: r.updated_on.with_timezone(&Utc),
            })
            .collect())
    }
}

/// One admin list's criteria: the words, the kind, the sector, the status.
fn admin_condition(filter: &AdminFilter) -> Condition {
    let mut cond = Condition::all().add(admin_post_search(filter.search()));
    if let Some(kind) = filter.kind.as_deref() {
        cond = cond.add(post::Column::Kind.eq(kind));
    }
    if let Some(sector) = filter.sector.as_deref() {
        cond = cond.add(post::Column::Sector.eq(sector));
    }
    if let Some(state) = filter.state {
        cond = cond.add(post::Column::Status.eq(state));
    }
    cond
}

/// A public list's criteria: published, and what `q` narrows to (a sector
/// or a province takes those of every sector, of the whole region too).
fn public_condition(q: &PostQuery) -> Condition {
    let mut cond = Condition::all().add(post::Column::Status.eq(PUBLISHED));
    if let Some(kind) = q.kind.as_deref() {
        cond = cond.add(post::Column::Kind.eq(kind));
    }
    if let Some(sector) = q.sector.as_deref() {
        cond = cond.add(
            Condition::any()
                .add(post::Column::Sector.eq(sector))
                .add(post::Column::Sector.is_null()),
        );
    }
    if let Some(province) = q.province_code.as_deref() {
        cond = cond.add(
            Condition::any()
                .add(post::Column::ProvinceCode.eq(province))
                .add(post::Column::ProvinceCode.is_null()),
        );
    }
    if let Some(commodity) = q.commodity_id {
        cond = cond.add(post::Column::CommodityId.eq(commodity));
    }
    if let Some(except) = q.except {
        cond = cond.add(post::Column::Id.ne(except));
    }
    cond
}

/// The public order: pinned first, the newest published first.
fn public_order(query: Select<PostEntity>) -> Select<PostEntity> {
    query
        .order_by_desc(post::Column::IsPinned)
        .order_by_desc(post::Column::PublishedOn)
        .order_by_desc(post::Column::Id)
}

#[async_trait]
impl PostRepositoryTrait for PostRepository {
    async fn admin_list(
        &self,
        filter: &AdminFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Post>, Error> {
        let query = PostEntity::find().filter(admin_condition(filter));
        let rows = admin_order(
            query,
            filter.sort,
            OrderColumns {
                created: post::Column::CreatedOn,
                name: post::Column::Title,
                id: post::Column::Id,
            },
            |q| {
                q.order_by_desc(post::Column::UpdatedOn)
                    .order_by_desc(post::Column::Id)
            },
        )
        .limit(limit as u64)
        .offset(offset as u64)
        .all(self.db.as_ref())
        .await
        .map_err(Self::fail("admin_list"))?;
        self.named(rows).await
    }

    async fn admin_count(&self, filter: &AdminFilter) -> Result<i64, Error> {
        PostEntity::find()
            .filter(admin_condition(filter))
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(Self::fail("admin_count"))
    }

    async fn find(&self, id: i32) -> Result<Option<Post>, Error> {
        let row = PostEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("find"))?;
        Ok(self.named(row.into_iter().collect()).await?.pop())
    }

    async fn create(&self, f: &PostFields, cover: Option<Uuid>, by: i32) -> Result<i32, Error> {
        PostEntity::insert(post::ActiveModel {
            kind: Set(f.kind.clone()),
            title: Set(f.title.clone()),
            summary: Set(f.summary.clone()),
            body: Set(f.body.clone()),
            cover_file_id: Set(cover),
            sector: Set(f.sector.clone()),
            province_code: Set(f.province_code.clone()),
            commodity_id: Set(f.commodity_id),
            is_pinned: Set(f.is_pinned),
            created_by: Set(Some(by)),
            updated_by: Set(Some(by)),
            ..Default::default()
        })
        .exec(self.db.as_ref())
        .await
        .map(|r| r.last_insert_id)
        .map_err(Self::fail("create"))
    }

    async fn update(&self, id: i32, f: &PostFields, by: i32) -> Result<bool, Error> {
        let result = PostEntity::update_many()
            .col_expr(post::Column::Kind, Expr::value(f.kind.clone()))
            .col_expr(post::Column::Title, Expr::value(f.title.clone()))
            .col_expr(post::Column::Summary, Expr::value(f.summary.clone()))
            .col_expr(post::Column::Body, Expr::value(f.body.clone()))
            .col_expr(post::Column::Sector, Expr::value(f.sector.clone()))
            .col_expr(
                post::Column::ProvinceCode,
                Expr::value(f.province_code.clone()),
            )
            .col_expr(post::Column::CommodityId, Expr::value(f.commodity_id))
            .col_expr(post::Column::IsPinned, Expr::value(f.is_pinned))
            .col_expr(post::Column::UpdatedBy, Expr::value(Some(by)))
            .filter(post::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(Self::fail("update"))?;
        Ok(result.rows_affected > 0)
    }

    async fn set_cover(
        &self,
        id: i32,
        cover: Option<Uuid>,
        by: i32,
    ) -> Result<Option<Option<Uuid>>, Error> {
        let Some(current) = PostEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("set_cover"))?
        else {
            return Ok(None);
        };
        PostEntity::update_many()
            .col_expr(post::Column::CoverFileId, Expr::value(cover))
            .col_expr(post::Column::UpdatedBy, Expr::value(Some(by)))
            .filter(post::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(Self::fail("set_cover"))?;
        Ok(Some(current.cover_file_id))
    }

    async fn set_status(&self, id: i32, status: &str, by: i32) -> Result<Option<bool>, Error> {
        let Some(current) = PostEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("set_status"))?
        else {
            return Ok(None);
        };
        if current.status == status {
            return Ok(Some(false));
        }
        let mut update = PostEntity::update_many()
            .col_expr(post::Column::Status, Expr::value(status))
            .col_expr(post::Column::UpdatedBy, Expr::value(Some(by)))
            .filter(post::Column::Id.eq(id));
        if status == PUBLISHED && current.published_on.is_none() {
            update = update.col_expr(post::Column::PublishedOn, Expr::current_timestamp().into());
        }
        update
            .exec(self.db.as_ref())
            .await
            .map_err(Self::fail("set_status"))?;
        Ok(Some(true))
    }

    async fn public_list(
        &self,
        query: &PostQuery,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Post>, Error> {
        let rows = public_order(PostEntity::find().filter(public_condition(query)))
            .limit(limit as u64)
            .offset(offset as u64)
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("public_list"))?;
        self.named(rows).await
    }

    async fn public_count(&self, query: &PostQuery) -> Result<i64, Error> {
        PostEntity::find()
            .filter(public_condition(query))
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(Self::fail("public_count"))
    }
}
