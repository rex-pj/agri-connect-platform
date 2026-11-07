//! The reads behind the demand report. The two lead sources are joined to
//! their business (demo data left out) and to the service or product they
//! were about, for its category, in one query each.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::{Alias, Expr, Func, Query, SelectStatement},
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter,
    QuerySelect,
};
use tracing::error;

use domain::{
    constants::{CALL_CHANNEL_ZALO, INQUIRY_STATUS_SPAM},
    models::demand::{BusinessArea, LeadChannel, LeadEvent, SoleCategory},
    repositories::demand_repository_trait::DemandRepositoryTrait,
    Error,
};

use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        business::{self, Entity as Business},
        call_click, inquiry, product,
        service::{self, Entity as ServiceEntity},
    },
    repositories::service_repository::live_public,
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
    location_id: Option<i32>,
}

#[derive(FromQueryResult)]
struct SoleRow {
    business_id: i32,
    category_id: i32,
}

/// `SELECT … FROM <table>` joined to its real business and, for the
/// category, to the service and product it names.
fn with_business_and_category<T, B, S, P>(
    table: T,
    business_id: B,
    service_id: S,
    product_id: P,
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
                Expr::col((service::Entity, service::Column::CategoryId)).into(),
                Expr::col((product::Entity, product::Column::CategoryId)).into(),
            ]),
            Alias::new("category_id"),
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
        .and_where(Expr::col((business::Entity, business::Column::IsDemo)).eq(false))
        .to_owned()
}

#[async_trait]
impl DemandRepositoryTrait for DemandRepository {
    async fn lead_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<LeadEvent>, Error> {
        let taps = with_business_and_category(
            call_click::Entity,
            call_click::Column::BusinessId,
            call_click::Column::ServiceId,
            call_click::Column::ProductId,
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

    async fn business_areas(&self, radius_m: f64) -> Result<Vec<BusinessArea>, Error> {
        // The nearest active preset in reach; the presets store plain lat/lng.
        let nearest = Expr::cust_with_values(
            r#"(SELECT l.id FROM locations l
                 WHERE l.is_active
                   AND ST_DWithin(ST_MakePoint(l.lng, l.lat)::geography, "businesses".location, $1)
                 ORDER BY ST_Distance(ST_MakePoint(l.lng, l.lat)::geography, "businesses".location), l.id
                 LIMIT 1)"#,
            [radius_m],
        );
        let rows = Business::find()
            .select_only()
            .column_as(business::Column::Id, "business_id")
            .expr_as(nearest, "location_id")
            .filter(Expr::cust(r#""businesses".location IS NOT NULL"#))
            .filter(business::Column::IsDemo.eq(false))
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
                r.location_id.map(|location_id| BusinessArea {
                    business_id: r.business_id,
                    location_id,
                })
            })
            .collect())
    }

    async fn sole_categories(&self) -> Result<Vec<SoleCategory>, Error> {
        live_public(ServiceEntity::find())
            .select_only()
            .column(service::Column::BusinessId)
            .column_as(service::Column::CategoryId.min(), "category_id")
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
}
