use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, JoinType,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set,
};
use std::sync::Arc;
use tracing::error;

use domain::{
    models::inquiry::{InquiryAdmin, NewInquiry},
    repositories::inquiry_repository_trait::InquiryRepositoryTrait,
};

use domain::{Error, ErrorCode};

use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        business,
        inquiry::{self, Entity as Inquiry},
    },
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
    id: i32,
    business_id: i32,
    business_name: String,
    requester_name: String,
    requester_phone: String,
    topic: String,
    status: String,
    created_on: DateTimeWithTimeZone,
    updated_on: DateTimeWithTimeZone,
    updated_by_name: Option<String>,
}

#[async_trait]
impl InquiryRepositoryTrait for InquiryRepository {
    async fn create(&self, inquiry: NewInquiry) -> Result<i32, Error> {
        let new_inquiry = inquiry::ActiveModel {
            business_id: Set(inquiry.business_id),
            service_id: Set(inquiry.service_id),
            topic: Set(inquiry.topic),
            requester_name: Set(inquiry.requester_name),
            requester_phone: Set(inquiry.requester_phone),
            note: Set(inquiry.note),
            ..Default::default()
        };

        Inquiry::insert(new_inquiry)
            .exec(self.db.as_ref())
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| {
                error!("inquiry create error: {e}");
                Error::internal("Failed to save contact request.")
            })
    }

    async fn count_admin(&self) -> Result<i64, Error> {
        Inquiry::find()
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("inquiry count_admin error: {e}");
                Error::internal("Failed to count inquiries.")
            })
    }

    async fn list_admin(&self, limit: i64, offset: i64) -> Result<Vec<InquiryAdmin>, Error> {
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
            .column(inquiry::Column::UpdatedOn)
            .expr_as(
                Expr::cust(
                    "(SELECT u.username FROM users u WHERE u.id = \"inquiries\".updated_by)",
                ),
                "updated_by_name",
            )
            .join(JoinType::InnerJoin, inquiry::Relation::Business.def())
            .order_by_desc(inquiry::Column::CreatedOn)
            .order_by_desc(inquiry::Column::Id)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<InquiryAdminRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| InquiryAdmin {
                        id: r.id,
                        business_id: r.business_id,
                        business_name: r.business_name,
                        requester_name: r.requester_name,
                        requester_phone: r.requester_phone,
                        topic: r.topic,
                        status: r.status,
                        created_on: r.created_on.with_timezone(&Utc),
                        updated_on: r.updated_on.with_timezone(&Utc),
                        updated_by_name: r.updated_by_name,
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("inquiry list_admin error: {e}");
                Error::internal("Failed to load inquiries.")
            })
    }

    async fn find_by_id_admin(&self, id: i32) -> Result<Option<InquiryAdmin>, Error> {
        Inquiry::find()
            .filter(inquiry::Column::Id.eq(id))
            .select_only()
            .column(inquiry::Column::Id)
            .column(inquiry::Column::BusinessId)
            .column_as(business::Column::Name, "business_name")
            .column(inquiry::Column::RequesterName)
            .column(inquiry::Column::RequesterPhone)
            .column(inquiry::Column::Topic)
            .column(inquiry::Column::Status)
            .column(inquiry::Column::CreatedOn)
            .column(inquiry::Column::UpdatedOn)
            .expr_as(
                Expr::cust(
                    "(SELECT u.username FROM users u WHERE u.id = \"inquiries\".updated_by)",
                ),
                "updated_by_name",
            )
            .join(JoinType::InnerJoin, inquiry::Relation::Business.def())
            .into_model::<InquiryAdminRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| InquiryAdmin {
                    id: r.id,
                    business_id: r.business_id,
                    business_name: r.business_name,
                    requester_name: r.requester_name,
                    requester_phone: r.requester_phone,
                    topic: r.topic,
                    status: r.status,
                    created_on: r.created_on.with_timezone(&Utc),
                    updated_on: r.updated_on.with_timezone(&Utc),
                    updated_by_name: r.updated_by_name,
                })
            })
            .map_err(|e| {
                error!("inquiry find_by_id_admin error: {e}");
                Error::internal("Failed to load inquiry.")
            })
    }

    async fn update_status(&self, id: i32, status: &str, admin_id: i32) -> Result<(), Error> {
        let result = Inquiry::update_many()
            .col_expr(inquiry::Column::Status, Expr::value(status.to_string()))
            .col_expr(inquiry::Column::UpdatedOn, Expr::current_timestamp().into())
            .col_expr(inquiry::Column::UpdatedBy, Expr::value(Some(admin_id)))
            .filter(inquiry::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("inquiry update_status error: {e}");
                Error::internal("Failed to update status.")
            })?;

        if result.rows_affected == 0 {
            return Err(Error::new(ErrorCode::InquiryNotFound));
        }
        Ok(())
    }
}
