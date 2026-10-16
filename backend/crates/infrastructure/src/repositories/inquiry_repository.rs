use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    FromQueryResult, JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    Set, Statement,
};
use std::sync::Arc;
use tracing::error;

use domain::{
    models::inquiry::{InquiryAdmin, InquiryForNotice, InquiryForOwner, NewInquiry},
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

    async fn find_for_notice(&self, id: i32) -> Result<Option<InquiryForNotice>, Error> {
        // The managing account's address, if it is live, else the business's.
        let row = InquiryNoticeRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            "SELECT b.name AS business_name, \
                    COALESCE(u.email, NULLIF(TRIM(b.email), '')) AS recipient_email, \
                    i.requester_name, i.requester_phone, i.topic, i.note \
             FROM inquiries i \
             JOIN businesses b ON b.id = i.business_id \
             LEFT JOIN users u ON u.id = b.owner_id \
                  AND u.is_active AND u.deleted_at IS NULL \
             WHERE i.id = $1",
            [id.into()],
        ))
        .one(self.db.as_ref())
        .await
        .map_err(|e| {
            error!("inquiry find_for_notice error: {e}");
            Error::internal("Failed to load inquiry.")
        })?;
        Ok(row.map(|r| InquiryForNotice {
            business_name: r.business_name,
            recipient_email: r.recipient_email,
            requester_name: r.requester_name,
            requester_phone: r.requester_phone,
            topic: r.topic,
            note: r.note,
        }))
    }

    async fn count_for_owner(&self, owner_user: i32) -> Result<(i64, i64), Error> {
        let row = OwnerCountsRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            format!(
                "SELECT count(*) AS total, count(*) FILTER (WHERE i.status = 'new') AS fresh \
                 {OWNED_INQUIRIES}"
            ),
            [owner_user.into()],
        ))
        .one(self.db.as_ref())
        .await
        .map_err(owner_error("count_for_owner"))?;
        Ok(row.map_or((0, 0), |r| (r.total, r.fresh)))
    }

    async fn list_for_owner(
        &self,
        owner_user: i32,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<InquiryForOwner>, Error> {
        let rows = InquiryForOwnerRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            format!(
                "SELECT i.id, s.name AS service_name, i.topic, i.requester_name, \
                        i.requester_phone, i.note, i.status, i.created_on \
                 {OWNED_INQUIRIES} \
                 ORDER BY i.created_on DESC, i.id DESC LIMIT $2 OFFSET $3"
            ),
            [owner_user.into(), limit.into(), offset.into()],
        ))
        .all(self.db.as_ref())
        .await
        .map_err(owner_error("list_for_owner"))?;
        Ok(rows
            .into_iter()
            .map(|r| InquiryForOwner {
                id: r.id,
                service_name: r.service_name,
                topic: r.topic,
                requester_name: r.requester_name,
                requester_phone: r.requester_phone,
                note: r.note,
                status: r.status,
                created_on: r.created_on.with_timezone(&Utc),
            })
            .collect())
    }

    async fn update_status_for_owner(
        &self,
        id: i32,
        owner_user: i32,
        status: &str,
    ) -> Result<bool, Error> {
        // The ownership check is in the same statement as the change.
        self.db
            .execute(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                "UPDATE inquiries i SET status = $3, updated_on = now(), updated_by = $2 \
                 FROM businesses b \
                 WHERE i.id = $1 AND b.id = i.business_id \
                   AND b.owner_id = $2 AND b.deleted_at IS NULL",
                [id.into(), owner_user.into(), status.into()],
            ))
            .await
            .map(|r| r.rows_affected() > 0)
            .map_err(|e| {
                error!("inquiry update_status_for_owner error: {e}");
                Error::internal("Failed to update status.")
            })
    }
}

#[derive(FromQueryResult)]
struct InquiryNoticeRow {
    business_name: String,
    recipient_email: Option<String>,
    requester_name: String,
    requester_phone: String,
    topic: String,
    note: Option<String>,
}

#[derive(FromQueryResult)]
struct OwnerCountsRow {
    total: i64,
    fresh: i64,
}

#[derive(FromQueryResult)]
struct InquiryForOwnerRow {
    id: i32,
    service_name: Option<String>,
    topic: String,
    requester_name: String,
    requester_phone: String,
    note: Option<String>,
    status: String,
    created_on: DateTimeWithTimeZone,
}

/// Inquiries to the live business `$1` owns, with the service each names.
/// Shared by the count and the list so the two can never disagree.
const OWNED_INQUIRIES: &str = "FROM inquiries i \
     JOIN businesses b ON b.id = i.business_id AND b.deleted_at IS NULL \
     LEFT JOIN services s ON s.id = i.service_id \
     WHERE b.owner_id = $1";

fn owner_error(what: &'static str) -> impl Fn(DbErr) -> Error {
    move |e| {
        error!("inquiry {what} error: {e}");
        Error::internal("Failed to load inquiries.")
    }
}
