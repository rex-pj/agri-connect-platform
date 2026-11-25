use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    sea_query::{Asterisk, Expr, Func, SimpleExpr},
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, FromQueryResult, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, QueryTrait, RelationTrait, Select, Set,
};
use std::sync::Arc;
use tracing::error;

use domain::{
    models::{
        admin_list::AdminFilter,
        business::ListingRef,
        inquiry::{InquiryAdmin, InquiryForNotice, InquiryForOwner, NewInquiry},
    },
    repositories::inquiry_repository_trait::InquiryRepositoryTrait,
};

use domain::{
    constants::{INQUIRY_STATUSES_REVIEWABLE, INQUIRY_STATUS_NEW, INQUIRY_STATUS_SPAM},
    Error, ErrorCode,
};

use crate::{
    admin_list::{admin_order, OrderColumns},
    db_types::DateTimeWithTimeZone,
    entities::{
        business,
        inquiry::{self, Entity as Inquiry},
        service, user,
    },
    search::admin_inquiry_search,
    sql::{account_name_of, businesses_of, live_owner_on, notice_email, QueryAs},
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
            product_id: Set(inquiry.product_id),
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

    async fn count_admin(&self, filter: &AdminFilter) -> Result<i64, Error> {
        Inquiry::find()
            .filter(admin_condition(filter))
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("inquiry count_admin error: {e}");
                Error::internal("Failed to count inquiries.")
            })
    }

    async fn count_for_listing(&self, listing: ListingRef) -> Result<i64, Error> {
        let (business_id, item) = match listing {
            ListingRef::Service { business_id, id } => {
                (business_id, inquiry::Column::ServiceId.eq(id))
            }
            ListingRef::Product { business_id, id } => {
                (business_id, inquiry::Column::ProductId.eq(id))
            }
        };
        Inquiry::find()
            .filter(inquiry::Column::BusinessId.eq(business_id))
            .filter(item)
            .filter(inquiry::Column::Status.ne(INQUIRY_STATUS_SPAM))
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| Error::internal(format!("inquiry count_for_listing: {e}")))
    }

    async fn find_reviewable(&self, business_id: i32, phone: &str) -> Result<Option<i32>, Error> {
        Inquiry::find()
            .filter(inquiry::Column::BusinessId.eq(business_id))
            .filter(inquiry::Column::RequesterPhone.eq(phone))
            .filter(inquiry::Column::Status.is_in(INQUIRY_STATUSES_REVIEWABLE.iter().copied()))
            .order_by_desc(inquiry::Column::CreatedOn)
            .order_by_desc(inquiry::Column::Id)
            .one(self.db.as_ref())
            .await
            .map(|row| row.map(|r| r.id))
            .map_err(|e| {
                error!("inquiry find_reviewable error: {e}");
                Error::internal("Failed to check the contact request.")
            })
    }

    async fn list_admin(
        &self,
        filter: &AdminFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<InquiryAdmin>, Error> {
        let query = Inquiry::find()
            .filter(admin_condition(filter))
            .select_only()
            .column(inquiry::Column::Id)
            .column(inquiry::Column::BusinessId)
            .column_as(business::Column::Name, QueryAs::BusinessName)
            .column(inquiry::Column::RequesterName)
            .column(inquiry::Column::RequesterPhone)
            .column(inquiry::Column::Topic)
            .column(inquiry::Column::Status)
            .column(inquiry::Column::CreatedOn)
            .column(inquiry::Column::UpdatedOn)
            .expr_as(
                account_name_of((inquiry::Entity, inquiry::Column::UpdatedBy)),
                QueryAs::UpdatedByName,
            )
            .join(JoinType::InnerJoin, inquiry::Relation::Business.def());
        admin_order(query, filter.sort, inquiry_order(), |q| {
            q.order_by_desc(inquiry::Column::CreatedOn)
                .order_by_desc(inquiry::Column::Id)
        })
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
            .column_as(business::Column::Name, QueryAs::BusinessName)
            .column(inquiry::Column::RequesterName)
            .column(inquiry::Column::RequesterPhone)
            .column(inquiry::Column::Topic)
            .column(inquiry::Column::Status)
            .column(inquiry::Column::CreatedOn)
            .column(inquiry::Column::UpdatedOn)
            .expr_as(
                account_name_of((inquiry::Entity, inquiry::Column::UpdatedBy)),
                QueryAs::UpdatedByName,
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
            .col_expr(inquiry::Column::AnsweredOn, answered_on(status))
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
        let mut query = Inquiry::find_by_id(id)
            .select_only()
            .column_as(business::Column::Name, QueryAs::BusinessName)
            .expr_as(notice_email(), QueryAs::RecipientEmail)
            .column(inquiry::Column::RequesterName)
            .column(inquiry::Column::RequesterPhone)
            .column(inquiry::Column::Topic)
            .column(inquiry::Column::Note)
            .join(JoinType::InnerJoin, inquiry::Relation::Business.def());
        QueryTrait::query(&mut query).left_join(user::Entity, live_owner_on());
        let row = query
            .into_model::<InquiryNoticeRow>()
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
        let row = owned_inquiries(owner_user)
            .select_only()
            .expr_as(Func::count(Expr::col(Asterisk)), QueryAs::Total)
            // COUNT skips the NULL the CASE gives every other status.
            .expr_as(
                Func::count(Expr::case(
                    inquiry::Column::Status.eq(INQUIRY_STATUS_NEW),
                    Expr::val(1),
                )),
                QueryAs::Fresh,
            )
            .into_model::<OwnerCountsRow>()
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
        let rows = owned_inquiries(owner_user)
            .select_only()
            .column(inquiry::Column::Id)
            .column_as(service::Column::Name, QueryAs::ServiceName)
            .column(inquiry::Column::Topic)
            .column(inquiry::Column::RequesterName)
            .column(inquiry::Column::RequesterPhone)
            .column(inquiry::Column::Note)
            .column(inquiry::Column::Status)
            .column(inquiry::Column::CreatedOn)
            .order_by_desc(inquiry::Column::CreatedOn)
            .order_by_desc(inquiry::Column::Id)
            .limit(u64::try_from(limit).unwrap_or(0))
            .offset(u64::try_from(offset).unwrap_or(0))
            .into_model::<InquiryForOwnerRow>()
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
        Inquiry::update_many()
            .col_expr(inquiry::Column::Status, Expr::value(status.to_owned()))
            .col_expr(inquiry::Column::UpdatedOn, Expr::current_timestamp().into())
            .col_expr(inquiry::Column::UpdatedBy, Expr::value(Some(owner_user)))
            .col_expr(inquiry::Column::AnsweredOn, answered_on(status))
            .filter(inquiry::Column::Id.eq(id))
            .filter(inquiry::Column::BusinessId.in_subquery(businesses_of(owner_user)))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
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

/// Inquiries to the live business `owner_user` owns, with the service each
/// names (left-joined). Shared by the count and the list so the two can
/// never disagree.
fn owned_inquiries(owner_user: i32) -> Select<Inquiry> {
    let mut query =
        Inquiry::find().filter(inquiry::Column::BusinessId.in_subquery(businesses_of(owner_user)));
    QueryTrait::query(&mut query).left_join(
        service::Entity,
        Expr::col((service::Entity, service::Column::Id))
            .equals((inquiry::Entity, inquiry::Column::ServiceId)),
    );
    query
}

fn owner_error(what: &'static str) -> impl Fn(DbErr) -> Error {
    move |e| {
        error!("inquiry {what} error: {e}");
        Error::internal("Failed to load inquiries.")
    }
}

/// The admin list's criteria, shared by the list and its count.
fn admin_condition(filter: &AdminFilter) -> sea_orm::Condition {
    admin_condition_base(filter).add(crate::admin_list::created_between(
        (
            crate::entities::inquiry::Entity,
            crate::entities::inquiry::Column::CreatedOn,
        ),
        filter,
    ))
}

fn admin_condition_base(filter: &AdminFilter) -> sea_orm::Condition {
    // The search is an OR of its columns: wrapped, so every other
    // criterion narrows it rather than joining the OR.
    let mut cond = sea_orm::Condition::all().add(admin_inquiry_search(filter.search()));
    if let Some(id) = filter.provider_id {
        cond = cond.add(inquiry::Column::BusinessId.eq(id));
    }
    if let Some(cutoff) = crate::admin_list::stale_cutoff(filter) {
        cond = cond
            .add(inquiry::Column::Status.eq(domain::constants::INQUIRY_STATUS_NEW))
            .add(inquiry::Column::CreatedOn.lt(cutoff));
    }
    match filter.kind.as_deref() {
        Some("service") => cond = cond.add(inquiry::Column::ServiceId.is_not_null()),
        Some("product") => cond = cond.add(inquiry::Column::ProductId.is_not_null()),
        _ => {}
    }
    if let Some(status) = filter.state {
        cond = cond.add(inquiry::Column::Status.eq(status));
    }
    cond
}

/// What the admin list orders by when asked (a name: the sender's).
fn inquiry_order() -> OrderColumns<inquiry::Column, inquiry::Column, inquiry::Column> {
    OrderColumns {
        created: inquiry::Column::CreatedOn,
        name: inquiry::Column::RequesterName,
        id: inquiry::Column::Id,
    }
}

/// `inquiries.answered_on` after a status change: the first time a request
/// is marked answered (`INQUIRY_STATUSES_REVIEWABLE`) is kept for good,
/// through a reopening and a later close; any other status leaves it.
fn answered_on(status: &str) -> SimpleExpr {
    let current = Expr::col(inquiry::Column::AnsweredOn);
    if INQUIRY_STATUSES_REVIEWABLE.contains(&status) {
        Func::coalesce([current.into(), Expr::current_timestamp().into()]).into()
    } else {
        current.into()
    }
}
