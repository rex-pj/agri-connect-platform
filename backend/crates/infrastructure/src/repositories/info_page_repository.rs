//! Information pages and the versions of their text.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use tracing::error;

use domain::{
    models::info_page::{InfoPage, InfoPageFields, InfoRevision, NewInfoRevision},
    repositories::info_page_repository_trait::InfoPageRepositoryTrait,
    Error, ErrorCode,
};

use crate::{
    entities::{
        info_page::{self, Entity as PageEntity},
        info_page_revision::{self, Entity as RevisionEntity},
    },
    repositories::unique_or_internal,
};

pub struct InfoPageRepository {
    db: Arc<DatabaseConnection>,
}

impl InfoPageRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    fn fail(what: &'static str) -> impl Fn(DbErr) -> Error {
        move |e| {
            error!("info page {what} error: {e}");
            Error::internal("Failed to load information pages.")
        }
    }
}

fn page(r: info_page::Model) -> InfoPage {
    InfoPage {
        id: r.id,
        code: r.code,
        title: r.title,
        icon: r.icon,
        stage: r.stage,
        in_footer: r.in_footer,
        footer_group: r.footer_group,
        display_order: r.display_order,
        is_demo: r.is_demo,
        updated_on: r.updated_on.with_timezone(&Utc),
    }
}

fn revision(r: info_page_revision::Model) -> InfoRevision {
    InfoRevision {
        id: r.id,
        page_id: r.page_id,
        body: r.body,
        effective_on: r.effective_on,
        change_note: r.change_note,
        created_on: r.created_on.with_timezone(&Utc),
    }
}

/// The footer's order: by group, then the admin's order, then the id.
fn ordered(query: sea_orm::Select<PageEntity>) -> sea_orm::Select<PageEntity> {
    query
        .order_by_asc(info_page::Column::FooterGroup)
        .order_by_asc(info_page::Column::DisplayOrder)
        .order_by_asc(info_page::Column::Id)
}

#[async_trait]
impl InfoPageRepositoryTrait for InfoPageRepository {
    async fn list_all(&self) -> Result<Vec<InfoPage>, Error> {
        Ok(ordered(PageEntity::find())
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("list"))?
            .into_iter()
            .map(page)
            .collect())
    }

    async fn with_stage(&self, stages: &[&str]) -> Result<Vec<InfoPage>, Error> {
        Ok(
            ordered(PageEntity::find().filter(info_page::Column::Stage.is_in(stages.to_vec())))
                .all(self.db.as_ref())
                .await
                .map_err(Self::fail("with_stage"))?
                .into_iter()
                .map(page)
                .collect(),
        )
    }

    async fn find(&self, id: i32) -> Result<Option<InfoPage>, Error> {
        Ok(PageEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("find"))?
            .map(page))
    }

    async fn find_by_code(&self, code: &str) -> Result<Option<InfoPage>, Error> {
        Ok(PageEntity::find()
            .filter(info_page::Column::Code.eq(code))
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("find_by_code"))?
            .map(page))
    }

    async fn create(&self, f: &InfoPageFields, by: i32) -> Result<i32, Error> {
        PageEntity::insert(info_page::ActiveModel {
            code: Set(f.code.clone()),
            title: Set(f.title.clone()),
            icon: Set(f.icon.clone()),
            in_footer: Set(f.in_footer),
            footer_group: Set(f.footer_group.clone()),
            display_order: Set(f.display_order),
            created_by: Set(Some(by)),
            updated_by: Set(Some(by)),
            ..Default::default()
        })
        .exec(self.db.as_ref())
        .await
        .map(|r| r.last_insert_id)
        .map_err(|e| {
            unique_or_internal(e, "code", ErrorCode::InfoPageCodeTaken, "info page create")
        })
    }

    async fn update(&self, id: i32, f: &InfoPageFields, by: i32) -> Result<bool, Error> {
        let result = PageEntity::update_many()
            .col_expr(info_page::Column::Title, Expr::value(f.title.clone()))
            .col_expr(info_page::Column::Icon, Expr::value(f.icon.clone()))
            .col_expr(info_page::Column::InFooter, Expr::value(f.in_footer))
            .col_expr(
                info_page::Column::FooterGroup,
                Expr::value(f.footer_group.clone()),
            )
            .col_expr(
                info_page::Column::DisplayOrder,
                Expr::value(f.display_order),
            )
            .col_expr(info_page::Column::UpdatedBy, Expr::value(Some(by)))
            .filter(info_page::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(Self::fail("update"))?;
        Ok(result.rows_affected > 0)
    }

    async fn set_stage(&self, id: i32, stage: &str, by: i32) -> Result<Option<bool>, Error> {
        let Some(current) = PageEntity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(Self::fail("set_stage"))?
        else {
            return Ok(None);
        };
        if current.stage == stage {
            return Ok(Some(false));
        }
        PageEntity::update_many()
            .col_expr(info_page::Column::Stage, Expr::value(stage))
            .col_expr(info_page::Column::UpdatedBy, Expr::value(Some(by)))
            .filter(info_page::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(Self::fail("set_stage"))?;
        Ok(Some(true))
    }

    async fn add_revision(&self, page_id: i32, r: &NewInfoRevision, by: i32) -> Result<i32, Error> {
        RevisionEntity::insert(info_page_revision::ActiveModel {
            page_id: Set(page_id),
            body: Set(r.body.clone()),
            effective_on: Set(r.effective_on),
            change_note: Set(r.change_note.clone()),
            created_by: Set(Some(by)),
            ..Default::default()
        })
        .exec(self.db.as_ref())
        .await
        .map(|r| r.last_insert_id)
        .map_err(Self::fail("add_revision"))
    }

    async fn revisions(&self, page_id: i32) -> Result<Vec<InfoRevision>, Error> {
        Ok(RevisionEntity::find()
            .filter(info_page_revision::Column::PageId.eq(page_id))
            .order_by_desc(info_page_revision::Column::EffectiveOn)
            .order_by_desc(info_page_revision::Column::Id)
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("revisions"))?
            .into_iter()
            .map(revision)
            .collect())
    }

    async fn with_text(&self, today: NaiveDate) -> Result<Vec<i32>, Error> {
        RevisionEntity::find()
            .select_only()
            .column(info_page_revision::Column::PageId)
            .distinct()
            .filter(info_page_revision::Column::EffectiveOn.lte(today))
            .into_tuple::<i32>()
            .all(self.db.as_ref())
            .await
            .map_err(Self::fail("with_text"))
    }
}
