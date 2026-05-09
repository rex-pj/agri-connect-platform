use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, JoinType, PaginatorTrait, QueryOrder, QuerySelect, RelationTrait, Set,
};
use std::sync::Arc;
use tracing::error;

use crate::{
    infrastructure::db_types::DateTimeWithTimeZone,
    domain::{
        models::review::ReviewAdmin,
        repositories::review_repository_trait::ReviewRepositoryTrait,
    },
    entities::{
        business,
        review::{self, Entity as Review},
    },
    view_models::AppError,
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
    id:            i32,
    business_id:   i32,
    business_name: String,
    reviewer_name: String,
    rating:        i16,
    comment:       Option<String>,
    is_visible:    bool,
    created_on:    DateTimeWithTimeZone,
}

#[async_trait]
impl ReviewRepositoryTrait for ReviewRepository {
    async fn count_admin(&self) -> Result<i64, AppError> {
        Review::find()
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("review count_admin error: {e}");
                AppError::Internal("Failed to count reviews.".into())
            })
    }

    async fn list_admin(&self, limit: i64, offset: i64) -> Result<Vec<ReviewAdmin>, AppError> {
        Review::find()
            .select_only()
            .column(review::Column::Id)
            .column(review::Column::BusinessId)
            .column_as(business::Column::Name, "business_name")
            .column(review::Column::ReviewerName)
            .column(review::Column::Rating)
            .column(review::Column::Comment)
            .column(review::Column::IsVisible)
            .column(review::Column::CreatedOn)
            .join(JoinType::InnerJoin, review::Relation::Business.def())
            .order_by_desc(review::Column::CreatedOn)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<ReviewAdminRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| ReviewAdmin {
                        id:            r.id,
                        business_id:   r.business_id,
                        business_name: r.business_name,
                        reviewer_name: r.reviewer_name,
                        rating:        r.rating,
                        comment:       r.comment,
                        is_visible:    r.is_visible,
                        created_on:    r.created_on.with_timezone(&Utc),
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("review list_admin error: {e}");
                AppError::Internal("Failed to load reviews.".into())
            })
    }

    async fn toggle_visible(&self, id: i32, is_visible: bool) -> Result<(), AppError> {
        let mut review: review::ActiveModel = Review::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("review toggle_visible find error: {e}");
                AppError::Internal("Failed to update review status.".into())
            })?
            .ok_or_else(|| AppError::NotFound("Review not found.".into()))?
            .into();

        review.is_visible = Set(is_visible);

        review
            .update(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("review toggle_visible update error: {e}");
                AppError::Internal("Failed to update review status.".into())
            })
    }
}

