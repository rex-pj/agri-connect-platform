use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    sea_query::{Expr, Func, SimpleExpr},
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, QueryTrait, RelationTrait, Select, Set,
};
use std::sync::Arc;
use tracing::error;

use domain::{
    models::review::{
        BusinessRating, NewReview, OwnReview, PublicReview, RatingSummary, ReviewAdmin,
    },
    repositories::review_repository_trait::ReviewRepositoryTrait,
};

use domain::{Error, ErrorCode};

use crate::repositories::unique_or_internal;
use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        business,
        review::{self, Entity as Review},
    },
    sql::{businesses_of, null, username_of, PgType, QueryAs},
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
    reply: Option<String>,
    is_visible: bool,
    created_on: DateTimeWithTimeZone,
    updated_on: DateTimeWithTimeZone,
    updated_by_name: Option<String>,
}

impl From<ReviewAdminRow> for ReviewAdmin {
    fn from(r: ReviewAdminRow) -> Self {
        Self {
            id: r.id,
            business_id: r.business_id,
            business_name: r.business_name,
            reviewer_name: r.reviewer_name,
            rating: r.rating,
            comment: r.comment,
            reply: r.reply,
            is_visible: r.is_visible,
            created_on: r.created_on.with_timezone(&Utc),
            updated_on: r.updated_on.with_timezone(&Utc),
            updated_by_name: r.updated_by_name,
        }
    }
}

#[derive(FromQueryResult)]
struct SummaryRow {
    count: i64,
    average: Option<f64>,
}

#[derive(FromQueryResult)]
struct RatingRow {
    business_id: i32,
    count: i64,
    average: f64,
}

/// The average rating: AVG of a SMALLINT is NUMERIC; as float8 it decodes
/// to f64.
fn average_rating() -> SimpleExpr {
    SimpleExpr::from(Func::avg(Expr::col(review::Column::Rating))).cast_as(PgType::Float8)
}

/// The columns of the admin list and detail, with the business's name.
fn admin_select() -> Select<Review> {
    Review::find()
        .select_only()
        .column(review::Column::Id)
        .column(review::Column::BusinessId)
        .column_as(business::Column::Name, QueryAs::BusinessName)
        .column(review::Column::ReviewerName)
        .column(review::Column::Rating)
        .column(review::Column::Comment)
        .column(review::Column::Reply)
        .column(review::Column::IsVisible)
        .column(review::Column::CreatedOn)
        .column(review::Column::UpdatedOn)
        .expr_as(
            username_of((review::Entity, review::Column::UpdatedBy)),
            QueryAs::UpdatedByName,
        )
        .join(JoinType::InnerJoin, review::Relation::Business.def())
}

/// Reviews of the live business `owner_user` manages: a subquery, so the
/// ownership check and the read or write are one statement.
fn owned_by(owner_user: i32) -> SimpleExpr {
    Expr::col((review::Entity, review::Column::BusinessId)).in_subquery(businesses_of(owner_user))
}

fn shown_of(business_id: i32) -> Select<Review> {
    Review::find()
        .filter(review::Column::BusinessId.eq(business_id))
        .filter(review::Column::IsVisible.eq(true))
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
        admin_select()
            .apply_if(provider_id, |q, id| {
                q.filter(review::Column::BusinessId.eq(id))
            })
            .order_by_desc(review::Column::CreatedOn)
            .order_by_desc(review::Column::Id)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<ReviewAdminRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(ReviewAdmin::from).collect())
            .map_err(|e| {
                error!("review list_admin error: {e}");
                Error::internal("Failed to load reviews.")
            })
    }

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<ReviewAdmin>, Error> {
        admin_select()
            .filter(review::Column::Id.eq(id))
            .into_model::<ReviewAdminRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| opt.map(ReviewAdmin::from))
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

    async fn create(&self, r: NewReview) -> Result<i32, Error> {
        Review::insert(review::ActiveModel {
            business_id: Set(r.business_id),
            inquiry_id: Set(Some(r.inquiry_id)),
            rating: Set(r.rating),
            comment: Set(r.comment),
            reviewer_name: Set(r.reviewer_name),
            reviewer_phone_hash: Set(r.reviewer_phone_hash),
            is_visible: Set(false),
            ..Default::default()
        })
        .exec(self.db.as_ref())
        .await
        .map(|res| res.last_insert_id)
        .map_err(|e| {
            unique_or_internal(
                e,
                "reviewer_phone",
                ErrorCode::ReviewAlreadySent,
                "review create",
            )
        })
    }

    async fn list_public(&self, business_id: i32, limit: i64) -> Result<Vec<PublicReview>, Error> {
        shown_of(business_id)
            .order_by_desc(review::Column::CreatedOn)
            .order_by_desc(review::Column::Id)
            .limit(limit as u64)
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| PublicReview {
                        id: r.id,
                        reviewer_name: r.reviewer_name,
                        rating: r.rating,
                        comment: r.comment,
                        reply: r.reply,
                        created_on: r.created_on.with_timezone(&Utc),
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("review list_public error: {e}");
                Error::internal("Failed to load reviews.")
            })
    }

    async fn summary_public(&self, business_id: i32) -> Result<RatingSummary, Error> {
        let average = average_rating();
        shown_of(business_id)
            .select_only()
            .column_as(review::Column::Id.count(), QueryAs::Count)
            .column_as(average, QueryAs::Average)
            .into_model::<SummaryRow>()
            .one(self.db.as_ref())
            .await
            .map(|row| {
                row.map_or(
                    RatingSummary {
                        count: 0,
                        average: None,
                    },
                    |r| RatingSummary {
                        count: r.count,
                        average: r.average,
                    },
                )
            })
            .map_err(|e| {
                error!("review summary_public error: {e}");
                Error::internal("Failed to load reviews.")
            })
    }

    async fn ratings_public(&self, business_ids: &[i32]) -> Result<Vec<BusinessRating>, Error> {
        if business_ids.is_empty() {
            return Ok(Vec::new());
        }
        Review::find()
            .filter(review::Column::BusinessId.is_in(business_ids.iter().copied()))
            .filter(review::Column::IsVisible.eq(true))
            .select_only()
            .column(review::Column::BusinessId)
            .column_as(review::Column::Id.count(), QueryAs::Count)
            .column_as(average_rating(), QueryAs::Average)
            .group_by(review::Column::BusinessId)
            .into_model::<RatingRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| BusinessRating {
                        business_id: r.business_id,
                        count: r.count,
                        average: r.average,
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("review ratings_public error: {e}");
                Error::internal("Failed to load reviews.")
            })
    }

    async fn list_for_owner(&self, owner_user: i32) -> Result<Vec<OwnReview>, Error> {
        Review::find()
            .filter(owned_by(owner_user))
            .order_by_desc(review::Column::CreatedOn)
            .order_by_desc(review::Column::Id)
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| OwnReview {
                        id: r.id,
                        reviewer_name: r.reviewer_name,
                        rating: r.rating,
                        comment: r.comment,
                        reply: r.reply,
                        is_visible: r.is_visible,
                        created_on: r.created_on.with_timezone(&Utc),
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("review list_for_owner error: {e}");
                Error::internal("Failed to load reviews.")
            })
    }

    async fn set_reply_for_owner(
        &self,
        owner_user: i32,
        id: i32,
        reply: Option<String>,
    ) -> Result<bool, Error> {
        let replied_at = if reply.is_some() {
            Expr::current_timestamp().into()
        } else {
            null()
        };
        Review::update_many()
            .col_expr(review::Column::Reply, Expr::value(reply))
            .col_expr(review::Column::RepliedAt, replied_at)
            .col_expr(review::Column::UpdatedOn, Expr::current_timestamp().into())
            .filter(review::Column::Id.eq(id))
            .filter(owned_by(owner_user))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(|e| {
                error!("review set_reply_for_owner error: {e}");
                Error::internal("Failed to save the reply.")
            })
    }
}
