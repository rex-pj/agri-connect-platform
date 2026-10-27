use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, JoinType,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait, RelationTrait,
};
use std::sync::Arc;
use tracing::error;

use domain::{
    models::review::ReviewAdmin, repositories::review_repository_trait::ReviewRepositoryTrait,
};

use domain::{Error, ErrorCode};

use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        business,
        review::{self, Entity as Review},
    },
};

pub struct ReviewRepository {
    db: Arc<DatabaseConnection>,
}

impl ReviewRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[derive(FromQueryResult)]
struct ReviewAdminRow {
    id: i32,
    business_id: i32,
    business_name: String,
    reviewer_name: String,
    rating: i16,
    comment: Option<String>,
    is_visible: bool,
    created_on: DateTimeWithTimeZone,
    updated_on: DateTimeWithTimeZone,
    updated_by_name: Option<String>,
}

#[async_trait]
impl ReviewRepositoryTrait for ReviewRepository {
    async fn count_admin(&self, provider_id: Option<i32>) -> Result<i64, Error> {
        Review::find()
            .apply_if(provider_id, |q, id| {
                q.filter(review::Column::BusinessId.eq(id))
            })
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("review count_admin error: {e}");
                Error::internal("Failed to count reviews.")
            })
    }

    async fn list_admin(
        &self,
        provider_id: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ReviewAdmin>, Error> {
        Review::find()
            .apply_if(provider_id, |q, id| {
                q.filter(review::Column::BusinessId.eq(id))
            })
            .select_only()
            .column(review::Column::Id)
            .column(review::Column::BusinessId)
            .column_as(business::Column::Name, "business_name")
            .column(review::Column::ReviewerName)
            .column(review::Column::Rating)
            .column(review::Column::Comment)
            .column(review::Column::IsVisible)
            .column(review::Column::CreatedOn)
            .column(review::Column::UpdatedOn)
            .expr_as(
                Expr::cust("(SELECT u.username FROM users u WHERE u.id = \"reviews\".updated_by)"),
                "updated_by_name",
            )
            .join(JoinType::InnerJoin, review::Relation::Business.def())
            .order_by_desc(review::Column::CreatedOn)
            .order_by_desc(review::Column::Id)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<ReviewAdminRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| ReviewAdmin {
                        id: r.id,
                        business_id: r.business_id,
                        business_name: r.business_name,
                        reviewer_name: r.reviewer_name,
                        rating: r.rating,
                        comment: r.comment,
                        is_visible: r.is_visible,
                        created_on: r.created_on.with_timezone(&Utc),
                        updated_on: r.updated_on.with_timezone(&Utc),
                        updated_by_name: r.updated_by_name,
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("review list_admin error: {e}");
                Error::internal("Failed to load reviews.")
            })
    }

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<ReviewAdmin>, Error> {
        Review::find()
            .filter(review::Column::Id.eq(id))
            .select_only()
            .column(review::Column::Id)
            .column(review::Column::BusinessId)
            .column_as(business::Column::Name, "business_name")
            .column(review::Column::ReviewerName)
            .column(review::Column::Rating)
            .column(review::Column::Comment)
            .column(review::Column::IsVisible)
            .column(review::Column::CreatedOn)
            .column(review::Column::UpdatedOn)
            .expr_as(
                Expr::cust("(SELECT u.username FROM users u WHERE u.id = \"reviews\".updated_by)"),
                "updated_by_name",
            )
            .join(JoinType::InnerJoin, review::Relation::Business.def())
            .into_model::<ReviewAdminRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| ReviewAdmin {
                    id: r.id,
                    business_id: r.business_id,
                    business_name: r.business_name,
                    reviewer_name: r.reviewer_name,
                    rating: r.rating,
                    comment: r.comment,
                    is_visible: r.is_visible,
                    created_on: r.created_on.with_timezone(&Utc),
                    updated_on: r.updated_on.with_timezone(&Utc),
                    updated_by_name: r.updated_by_name,
                })
            })
            .map_err(|e| {
                error!("review find_by_id_admin error: {e}");
                Error::internal("Failed to load review.")
            })
    }

    async fn toggle_visible(&self, id: i32, is_visible: bool, admin_id: i32) -> Result<(), Error> {
        let result = Review::update_many()
            .col_expr(review::Column::IsVisible, Expr::value(is_visible))
            .col_expr(review::Column::UpdatedOn, Expr::current_timestamp().into())
            .col_expr(review::Column::UpdatedBy, Expr::value(Some(admin_id)))
            .filter(review::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("review toggle_visible error: {e}");
                Error::internal("Failed to update review status.")
            })?;

        if result.rows_affected == 0 {
            return Err(Error::new(ErrorCode::ReviewNotFound));
        }
        Ok(())
    }
}
