//! The reads behind the demand report. The two lead sources are joined to
//! their business (in the report's `DemoScope`) and to the service or product they
//! were about, for its category, in one query each.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::{Condition, Expr, Func, Query, SelectStatement},
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, FromQueryResult, PaginatorTrait,
    QueryFilter, QuerySelect, Set,
};
use tracing::error;

use domain::{
    constants::{CALL_CHANNEL_ZALO, INQUIRY_STATUS_SPAM},
    models::demand::{
        BusinessArea, DemoScope, LeadChannel, LeadEvent, NewSearch, SearchEvent, SoleCategory,
    },
    repositories::demand_repository_trait::DemandRepositoryTrait,
    Error,
};

use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        business::{self, Entity as Business},
        call_click, inquiry, product, search_event,
        service::{self, Entity as ServiceEntity},
        service_type,
    },
    repositories::{admin_unit_repository::ward_code_at, service_repository::live_public},
    sql::{business_location, QueryAs},
};

pub struct DemandRepository {
    db: Arc<DatabaseConnection>,
}

impl DemandRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    async fn rows<T: FromQueryResult>(
        &self,
        query: &SelectStatement,
        what: &str,
    ) -> Result<Vec<T>, Error> {
        let db = self.db.as_ref();
        T::find_by_statement(db.get_database_backend().build(query))
            .all(db)
            .await
            .map_err(|e| {
                error!("demand {what} error: {e}");
                Error::internal("Failed to load the demand report.")
            })
    }
}

#[derive(FromQueryResult)]
struct TapRow {
    created_on: DateTimeWithTimeZone,
    business_id: i32,
    category_id: Option<i32>,
    channel: String,
}

#[derive(FromQueryResult)]
struct InquiryRow {
    created_on: DateTimeWithTimeZone,
    business_id: i32,
    category_id: Option<i32>,
    requester_phone: String,
}

#[derive(FromQueryResult)]
struct AreaRow {
    business_id: i32,
    ward_code: Option<String>,
    name: String,
    phone: String,
    address: Option<String>,
    is_demo: bool,
}

#[derive(FromQueryResult)]
struct SoleRow {
    business_id: i32,
    category_id: i32,
}

/// `businesses.is_demo` as `scope` counts it: one value, or any.
fn business_in(scope: DemoScope) -> Condition {
    match scope.only() {
        Some(demo) => {
            Condition::all().add(Expr::col((business::Entity, business::Column::IsDemo)).eq(demo))
        }
        None => Condition::all(),
    }
}

/// `SELECT … FROM <table>` joined to its business in `scope` and, for the
/// category, to the service and product it names.
fn with_business_and_category<T, B, S, P>(
    table: T,
    business_id: B,
    service_id: S,
    product_id: P,
    scope: DemoScope,
) -> SelectStatement
where
    T: sea_orm::Iden + Copy + 'static,
    B: sea_orm::Iden + 'static,
    S: sea_orm::Iden + 'static,
    P: sea_orm::Iden + 'static,
{
    Query::select()
        .from(table)
        .expr_as(
            Func::coalesce([
                Expr::col((service::Entity, service::Column::ServiceTypeId)).into(),
                Expr::col((product::Entity, product::Column::SupplyTypeId)).into(),
            ]),
            QueryAs::CategoryId,
        )
        .inner_join(
            business::Entity,
            Expr::col((business::Entity, business::Column::Id)).equals((table, business_id)),
        )
        .left_join(
            service::Entity,
            Expr::col((service::Entity, service::Column::Id)).equals((table, service_id)),
        )
        .left_join(
            product::Entity,
            Expr::col((product::Entity, product::Column::Id)).equals((table, product_id)),
        )
        .cond_where(business_in(scope))
        .to_owned()
}

#[async_trait]
impl DemandRepositoryTrait for DemandRepository {
    async fn lead_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        scope: DemoScope,
    ) -> Result<Vec<LeadEvent>, Error> {
        let taps = with_business_and_category(
            call_click::Entity,
            call_click::Column::BusinessId,
            call_click::Column::ServiceId,
            call_click::Column::ProductId,
            scope,
        )
        .column((call_click::Entity, call_click::Column::CreatedOn))
        .column((call_click::Entity, call_click::Column::BusinessId))
        .column((call_click::Entity, call_click::Column::Channel))
        .and_where(Expr::col((call_click::Entity, call_click::Column::CreatedOn)).gte(from))
        .and_where(Expr::col((call_click::Entity, call_click::Column::CreatedOn)).lt(to))
        .to_owned();
        let inquiries = with_business_and_category(
            inquiry::Entity,
            inquiry::Column::BusinessId,
            inquiry::Column::ServiceId,
            inquiry::Column::ProductId,
            scope,
        )
        .column((inquiry::Entity, inquiry::Column::CreatedOn))
        .column((inquiry::Entity, inquiry::Column::BusinessId))
        .column((inquiry::Entity, inquiry::Column::RequesterPhone))
        .and_where(Expr::col((inquiry::Entity, inquiry::Column::CreatedOn)).gte(from))
        .and_where(Expr::col((inquiry::Entity, inquiry::Column::CreatedOn)).lt(to))
        .and_where(Expr::col((inquiry::Entity, inquiry::Column::Status)).ne(INQUIRY_STATUS_SPAM))
        .to_owned();
        let (taps, inquiries) = tokio::try_join!(
            self.rows::<TapRow>(&taps, "taps"),
            self.rows::<InquiryRow>(&inquiries, "inquiries"),
        )?;
        let mut events: Vec<LeadEvent> = taps
            .into_iter()
            .map(|r| LeadEvent {
                at: r.created_on.with_timezone(&Utc),
                business_id: r.business_id,
                category_id: r.category_id,
                channel: if r.channel == CALL_CHANNEL_ZALO {
                    LeadChannel::Zalo
                } else {
                    LeadChannel::Call
                },
                requester_phone: None,
            })
            .collect();
        events.extend(inquiries.into_iter().map(|r| LeadEvent {
            at: r.created_on.with_timezone(&Utc),
            business_id: r.business_id,
            category_id: r.category_id,
            channel: LeadChannel::Inquiry,
            requester_phone: Some(r.requester_phone),
        }));
        Ok(events)
    }

    async fn business_areas(
        &self,
        snap_m: f64,
        scope: DemoScope,
    ) -> Result<Vec<BusinessArea>, Error> {
        let rows = Business::find()
            .select_only()
            .column_as(business::Column::Id, QueryAs::BusinessId)
            .column(business::Column::Name)
            .column(business::Column::Phone)
            .column(business::Column::Address)
            .column(business::Column::IsDemo)
            .expr_as(ward_code_at(business_location(), snap_m), QueryAs::WardCode)
            .filter(Expr::expr(business_location()).is_not_null())
            .filter(business_in(scope))
            .filter(business::Column::DeletedAt.is_null())
            .into_model::<AreaRow>()
            .all(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("demand business_areas error: {e}");
                Error::internal("Failed to load the demand report.")
            })?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let (name, phone, address, is_demo) = (r.name, r.phone, r.address, r.is_demo);
                r.ward_code.map(|ward_code| BusinessArea {
                    name,
                    phone,
                    address,
                    business_id: r.business_id,
                    ward_code,
                    is_demo,
                })
            })
            .collect())
    }

    async fn sole_categories(&self) -> Result<Vec<SoleCategory>, Error> {
        live_public(ServiceEntity::find())
            .select_only()
            .column(service::Column::BusinessId)
            .column_as(service::Column::ServiceTypeId.min(), QueryAs::CategoryId)
            .group_by(service::Column::BusinessId)
            .having(service::Column::Id.count().eq(1))
            .into_model::<SoleRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| SoleCategory {
                        business_id: r.business_id,
                        category_id: r.category_id,
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("demand sole_categories error: {e}");
                Error::internal("Failed to load the demand report.")
            })
    }

    async fn record_search(&self, s: NewSearch) -> Result<(), Error> {
        // One id sequence for both type tables: the id says which it is.
        let is_service = match s.category_id {
            Some(id) => {
                service_type::Entity::find_by_id(id)
                    .count(self.db.as_ref())
                    .await
                    .map_err(|e| {
                        error!("demand record_search error: {e}");
                        Error::internal("Failed to record the search.")
                    })?
                    > 0
            }
            None => false,
        };
        let (service_type_id, supply_type_id) = match s.category_id {
            Some(id) if is_service => (Some(id), None),
            other => (None, other),
        };
        search_event::Entity::insert(search_event::ActiveModel {
            query: Set(s.query),
            service_type_id: Set(service_type_id),
            supply_type_id: Set(supply_type_id),
            ward_code: Set(s.ward_code),
            results: Set(i32::try_from(s.results).unwrap_or(i32::MAX)),
            ..Default::default()
        })
        .exec(self.db.as_ref())
        .await
        .map(|_| ())
        .map_err(|e| {
            error!("demand record_search error: {e}");
            Error::internal("Failed to record the search.")
        })
    }

    async fn search_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        scope: DemoScope,
    ) -> Result<Vec<SearchEvent>, Error> {
        let mut query = search_event::Entity::find()
            .filter(search_event::Column::CreatedOn.gte(from))
            .filter(search_event::Column::CreatedOn.lt(to));
        if let Some(demo) = scope.only() {
            query = query.filter(search_event::Column::IsDemo.eq(demo));
        }
        query
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| SearchEvent {
                        at: r.created_on.with_timezone(&Utc),
                        query: r.query,
                        category_id: r.service_type_id.or(r.supply_type_id),
                        ward_code: r.ward_code,
                        results: i64::from(r.results),
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("demand search_events error: {e}");
                Error::internal("Failed to load the demand report.")
            })
    }
}
