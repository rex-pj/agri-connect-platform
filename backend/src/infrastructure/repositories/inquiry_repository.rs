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
        models::inquiry::{InquiryAdmin, NewInquiry},
        repositories::inquiry_repository_trait::InquiryRepositoryTrait,
    },
    entities::{
        business,
        inquiry::{self, Entity as Inquiry},
    },
    view_models::AppError,
};

pub struct InquiryRepository {
    db: Arc<DatabaseConnection>,
}

impl InquiryRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[derive(FromQueryResult)]
struct InquiryAdminRow {
    id:              i32,
    business_id:     i32,
    business_name:   String,
    requester_name:  String,
    requester_phone: String,
    topic:           String,
    status:          String,
    created_on:      DateTimeWithTimeZone,
}

#[async_trait]
impl InquiryRepositoryTrait for InquiryRepository {
    async fn create(&self, inquiry: NewInquiry) -> Result<(), AppError> {
        let new_inquiry = inquiry::ActiveModel {
            business_id:     Set(inquiry.business_id),
            service_id:      Set(inquiry.service_id),
            topic:           Set(inquiry.topic),
            requester_name:  Set(inquiry.requester_name),
            requester_phone: Set(inquiry.requester_phone),
            note:            Set(inquiry.note),
            ..Default::default()
        };

        Inquiry::insert(new_inquiry)
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("inquiry create error: {e}");
                AppError::Internal("Failed to save contact request.".into())
            })
    }

    async fn count_admin(&self) -> Result<i64, AppError> {
        Inquiry::find()
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("inquiry count_admin error: {e}");
                AppError::Internal("Failed to count inquiries.".into())
            })
    }

    async fn list_admin(&self, limit: i64, offset: i64) -> Result<Vec<InquiryAdmin>, AppError> {
        Inquiry::find()
            .select_only()
            .column(inquiry::Column::Id)
            .column(inquiry::Column::BusinessId)
            .column_as(business::Column::Name, "business_name")
            .column(inquiry::Column::RequesterName)
            .column(inquiry::Column::RequesterPhone)
            .column(inquiry::Column::Topic)
            .column(inquiry::Column::Status)
            .column(inquiry::Column::CreatedOn)
            .join(JoinType::InnerJoin, inquiry::Relation::Business.def())
            .order_by_desc(inquiry::Column::CreatedOn)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<InquiryAdminRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| InquiryAdmin {
                        id:              r.id,
                        business_id:     r.business_id,
                        business_name:   r.business_name,
                        requester_name:  r.requester_name,
                        requester_phone: r.requester_phone,
                        topic:           r.topic,
                        status:          r.status,
                        created_on:      r.created_on.with_timezone(&Utc),
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("inquiry list_admin error: {e}");
                AppError::Internal("Failed to load inquiries.".into())
            })
    }

    async fn update_status(&self, id: i32, status: &str) -> Result<(), AppError> {
        let mut inquiry: inquiry::ActiveModel = Inquiry::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("inquiry update_status find error: {e}");
                AppError::Internal("Failed to update status.".into())
            })?
            .ok_or_else(|| AppError::NotFound("Inquiry not found.".into()))?
            .into();

        inquiry.status = Set(status.to_string());

        inquiry
            .update(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("inquiry update_status error: {e}");
                AppError::Internal("Failed to update status.".into())
            })
    }
}

