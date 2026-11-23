//! The reads behind the demand report. The database does the counting: the
//! two lead sources are joined to their business (in the report's
//! `DemoScope`) and to the service or product they were about, for its
//! category, and grouped by report week, so a report reads a few rows per
//! business and never every tap. A business's ward is the one stored with
//! its position.
//!
//! Taps and searches older than any report are counted into weekly totals
//! and deleted (`roll_up_week`), so the two tables stop growing.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::{
    sea_query::{
        Asterisk, BinOper, Condition, Expr, Func, IntoColumnRef, Query, SelectStatement, SimpleExpr,
    },
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait,
    FromQueryResult, PaginatorTrait, QueryFilter, QuerySelect, Set, StatementBuilder,
    TransactionTrait,
};
use tracing::error;

use domain::{
    constants::{CALL_CHANNEL_ZALO, INQUIRY_STATUS_SPAM},
    models::demand::{DemoScope, LeadChannel, LeadCount, NewSearch, SearchCount, SoleCategory},
    repositories::demand_repository_trait::DemandRepositoryTrait,
    Error,
};

use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        business::{self, Entity as Business},
        call_click, call_click_week, inquiry, product, search_event, search_event_week,
        service::{self, Entity as ServiceEntity},
        service_type,
    },
    repositories::service_repository::live_public,
    sql::{advisory_xact_lock, null, output, DbFn, PgType, QueryAs},
};

/// A report week, in seconds: Vietnam keeps one offset all year, so every
/// week from a Monday midnight there is exactly this long.
const WEEK_SECS: f64 = 7.0 * 24.0 * 3600.0;

/// Two processes rolling the same week up count it once.
const ROLL_UP_LOCK_ID: i64 = 0x6167_7269_5f72_6f6c; // "agri_rol"

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
    week: i32,
    area: Option<String>,
    category_id: Option<i32>,
    shop_id: Option<i32>,
    channel: String,
    count: i64,
}

#[derive(FromQueryResult)]
struct InquiryRow {
    week: i32,
    area: Option<String>,
    category_id: Option<i32>,
    shop_id: Option<i32>,
    requester_phone: String,
    count: i64,
}

#[derive(FromQueryResult)]
struct SearchRow {
    week: i32,
    words: Option<String>,
    service_type_id: Option<i32>,
    supply_type_id: Option<i32>,
    ward_code: Option<String>,
    unmet: bool,
    count: i64,
}

#[derive(FromQueryResult)]
struct SoleRow {
    business_id: i32,
    category_id: i32,
}

/// `businesses.is_demo` as `scope` counts it: one value, or any.
pub(crate) fn business_in(scope: DemoScope) -> Condition {
    match scope.only() {
        Some(demo) => {
            Condition::all().add(Expr::col((business::Entity, business::Column::IsDemo)).eq(demo))
        }
        None => Condition::all(),
    }
}

/// `COALESCE(services.service_type_id, products.supply_type_id)`: the
/// category of what a lead was about.
fn lead_category() -> SimpleExpr {
    Func::coalesce([
        Expr::col((service::Entity, service::Column::ServiceTypeId)).into(),
        Expr::col((product::Entity, product::Column::SupplyTypeId)).into(),
    ])
    .into()
}

/// `SELECT … FROM <table>` joined to its business in `scope` and, for the
/// category, to the service and product it names. What a report groups by:
/// `area` (the ward of the business while it is not deleted), `category_id`,
/// and `shop_id` (the business, only where there is no category).
fn with_business_and_category<T, B, S, P>(
    table: T,
    business_id: B,
    service_id: S,
    product_id: P,
    scope: DemoScope,
) -> SelectStatement
where
    T: sea_orm::Iden + Copy + 'static,
    B: sea_orm::Iden + Copy + 'static,
    S: sea_orm::Iden + 'static,
    P: sea_orm::Iden + 'static,
{
    Query::select()
        .from(table)
        .expr_as(lead_category(), QueryAs::CategoryId)
        .expr_as(
            Expr::case(
                Expr::col((business::Entity, business::Column::DeletedAt)).is_null(),
                Expr::col((business::Entity, business::Column::WardCode)),
            )
            .finally(null()),
            QueryAs::Area,
        )
        .expr_as(
            Expr::case(
                Expr::expr(lead_category()).is_null(),
                Expr::col((table, business_id)),
            )
            .finally(null()),
            QueryAs::ShopId,
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

/// The whole weeks between `from` and `created_on`, 0 for the first:
/// `floor(date_part('epoch', created_on - from) / week)::int4`.
fn week_since<C: IntoColumnRef>(created_on: C, from: DateTime<Utc>) -> SimpleExpr {
    let since: SimpleExpr = Expr::col(created_on).sub(Expr::val(from));
    let seconds: SimpleExpr = Func::cust(DbFn::DatePart)
        .args([Expr::val("epoch").into(), since])
        .into();
    SimpleExpr::from(
        Func::cust(DbFn::Floor).arg(seconds.binary(BinOper::Div, Expr::val(WEEK_SECS))),
    )
    .cast_as(PgType::Int4)
}

/// `created_on` in `[from, to)`.
fn within<C: IntoColumnRef + Clone>(
    created_on: C,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Condition {
    Condition::all()
        .add(Expr::col(created_on.clone()).gte(from))
        .add(Expr::col(created_on).lt(to))
}

/// `COUNT(*)::int4`: a week's count as the weekly totals store it.
fn count_int() -> SimpleExpr {
    SimpleExpr::from(Func::count(Expr::col(Asterisk))).cast_as(PgType::Int4)
}

/// A report week from the database, as an index. A window that starts at
/// `from` has no week before the first.
fn week_index(week: i32) -> Option<usize> {
    usize::try_from(week).ok()
}

async fn run<S: StatementBuilder>(
    txn: &DatabaseTransaction,
    step: &'static str,
    statement: &S,
) -> Result<u64, Error> {
    txn.execute(txn.get_database_backend().build(statement))
        .await
        .map(|r| r.rows_affected())
        .map_err(roll_up_error(step))
}

fn roll_up_error(step: &'static str) -> impl Fn(DbErr) -> Error {
    move |e| {
        error!("demand roll-up {step} error: {e}");
        Error::internal("Failed to roll up old demand events.")
    }
}

#[async_trait]
impl DemandRepositoryTrait for DemandRepository {
    async fn lead_counts(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        scope: DemoScope,
    ) -> Result<Vec<LeadCount>, Error> {
        let tapped = (call_click::Entity, call_click::Column::CreatedOn);
        let taps = with_business_and_category(
            call_click::Entity,
            call_click::Column::BusinessId,
            call_click::Column::ServiceId,
            call_click::Column::ProductId,
            scope,
        )
        .expr_as(week_since(tapped, from), QueryAs::Week)
        .column((call_click::Entity, call_click::Column::Channel))
        .expr_as(
            Expr::col((call_click::Entity, call_click::Column::Id)).count(),
            QueryAs::Count,
        )
        .cond_where(within(tapped, from, to))
        // By output name: the week is an expression with bound values, and
        // Postgres cannot tell two spellings of it are the same.
        .add_group_by([
            output(QueryAs::Week),
            output(QueryAs::Area),
            output(QueryAs::CategoryId),
            output(QueryAs::ShopId),
            Expr::col((call_click::Entity, call_click::Column::Channel)).into(),
        ])
        .to_owned();
        let sent = (inquiry::Entity, inquiry::Column::CreatedOn);
        let inquiries = with_business_and_category(
            inquiry::Entity,
            inquiry::Column::BusinessId,
            inquiry::Column::ServiceId,
            inquiry::Column::ProductId,
            scope,
        )
        .expr_as(week_since(sent, from), QueryAs::Week)
        .column((inquiry::Entity, inquiry::Column::RequesterPhone))
        .expr_as(
            Expr::col((inquiry::Entity, inquiry::Column::Id)).count(),
            QueryAs::Count,
        )
        .cond_where(within(sent, from, to))
        .and_where(Expr::col((inquiry::Entity, inquiry::Column::Status)).ne(INQUIRY_STATUS_SPAM))
        .add_group_by([
            output(QueryAs::Week),
            output(QueryAs::Area),
            output(QueryAs::CategoryId),
            output(QueryAs::ShopId),
            Expr::col((inquiry::Entity, inquiry::Column::RequesterPhone)).into(),
        ])
        .to_owned();
        let (taps, inquiries) = tokio::try_join!(
            self.rows::<TapRow>(&taps, "taps"),
            self.rows::<InquiryRow>(&inquiries, "inquiries"),
        )?;
        let taps = taps.into_iter().filter_map(|r| {
            Some(LeadCount {
                week: week_index(r.week)?,
                ward_code: r.area,
                category_id: r.category_id,
                business_id: r.shop_id,
                channel: if r.channel == CALL_CHANNEL_ZALO {
                    LeadChannel::Zalo
                } else {
                    LeadChannel::Call
                },
                count: r.count,
                requester_phone: None,
            })
        });
        let inquiries = inquiries.into_iter().filter_map(|r| {
            Some(LeadCount {
                week: week_index(r.week)?,
                ward_code: r.area,
                category_id: r.category_id,
                business_id: r.shop_id,
                channel: LeadChannel::Inquiry,
                count: r.count,
                requester_phone: Some(r.requester_phone),
            })
        });
        Ok(taps.chain(inquiries).collect())
    }

    async fn any_demo_business(&self) -> Result<bool, Error> {
        Business::find()
            .filter(business::Column::IsDemo.eq(true))
            .filter(business::Column::DeletedAt.is_null())
            .one(self.db.as_ref())
            .await
            .map(|b| b.is_some())
            .map_err(|e| {
                error!("demand any_demo_business error: {e}");
                Error::internal("Failed to load the demand report.")
            })
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

    async fn search_counts(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        scope: DemoScope,
    ) -> Result<Vec<SearchCount>, Error> {
        let found_nothing = || Expr::col(search_event::Column::Results).eq(0);
        let mut query = search_event::Entity::find()
            .select_only()
            .expr_as(
                week_since(search_event::Column::CreatedOn, from),
                QueryAs::Week,
            )
            // The words only of what found nothing: the searches that found
            // something are one row per week, filter and ward.
            .expr_as(
                Expr::case(found_nothing(), Expr::col(search_event::Column::Query)).finally(null()),
                QueryAs::Words,
            )
            .column(search_event::Column::ServiceTypeId)
            .column(search_event::Column::SupplyTypeId)
            .column(search_event::Column::WardCode)
            .expr_as(found_nothing(), QueryAs::Unmet)
            .expr_as(search_event::Column::Id.count(), QueryAs::Count)
            .filter(within(search_event::Column::CreatedOn, from, to))
            .group_by(output(QueryAs::Week))
            .group_by(output(QueryAs::Words))
            .group_by(search_event::Column::ServiceTypeId)
            .group_by(search_event::Column::SupplyTypeId)
            .group_by(search_event::Column::WardCode)
            .group_by(output(QueryAs::Unmet));
        if let Some(demo) = scope.only() {
            query = query.filter(search_event::Column::IsDemo.eq(demo));
        }
        query
            .into_model::<SearchRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .filter_map(|r| {
                        Some(SearchCount {
                            week: week_index(r.week)?,
                            query: r.words,
                            category_id: r.service_type_id.or(r.supply_type_id),
                            ward_code: r.ward_code,
                            unmet: r.unmet,
                            count: r.count,
                        })
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("demand search_counts error: {e}");
                Error::internal("Failed to load the demand report.")
            })
    }

    async fn oldest_event_before(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, Error> {
        let db = self.db.as_ref();
        let (tap, search) = tokio::try_join!(
            call_click::Entity::find()
                .select_only()
                .column_as(call_click::Column::CreatedOn.min(), QueryAs::Oldest)
                .filter(call_click::Column::CreatedOn.lt(cutoff))
                .into_tuple::<Option<DateTimeWithTimeZone>>()
                .one(db),
            search_event::Entity::find()
                .select_only()
                .column_as(search_event::Column::CreatedOn.min(), QueryAs::Oldest)
                .filter(search_event::Column::CreatedOn.lt(cutoff))
                .into_tuple::<Option<DateTimeWithTimeZone>>()
                .one(db),
        )
        .map_err(roll_up_error("oldest event"))?;
        Ok([tap, search]
            .into_iter()
            .filter_map(Option::flatten)
            .map(|t| t.with_timezone(&Utc))
            .min())
    }

    async fn roll_up_week(
        &self,
        week_starts_on: NaiveDate,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<u64, Error> {
        let txn = self.db.begin().await.map_err(roll_up_error("begin"))?;
        run(&txn, "lock", &advisory_xact_lock(ROLL_UP_LOCK_ID)).await?;

        // A tap's type is its service's or its supply's, as the report reads it.
        let tapped = (call_click::Entity, call_click::Column::CreatedOn);
        let types = || -> [SimpleExpr; 2] {
            [
                Expr::col((service::Entity, service::Column::ServiceTypeId)).into(),
                Expr::col((product::Entity, product::Column::SupplyTypeId)).into(),
            ]
        };
        let tap_weeks = Query::select()
            .expr(Expr::val(week_starts_on))
            .column((call_click::Entity, call_click::Column::BusinessId))
            .exprs(types())
            .column((call_click::Entity, call_click::Column::Channel))
            .expr(count_int())
            .from(call_click::Entity)
            .left_join(
                service::Entity,
                Expr::col((service::Entity, service::Column::Id))
                    .equals((call_click::Entity, call_click::Column::ServiceId)),
            )
            .left_join(
                product::Entity,
                Expr::col((product::Entity, product::Column::Id))
                    .equals((call_click::Entity, call_click::Column::ProductId)),
            )
            .cond_where(within(tapped, from, to))
            .add_group_by(
                [
                    Expr::col((call_click::Entity, call_click::Column::BusinessId)).into(),
                    Expr::col((call_click::Entity, call_click::Column::Channel)).into(),
                ]
                .into_iter()
                .chain(types()),
            )
            .to_owned();
        let mut insert_taps = Query::insert()
            .into_table(call_click_week::Entity)
            .columns([
                call_click_week::Column::WeekStartsOn,
                call_click_week::Column::BusinessId,
                call_click_week::Column::ServiceTypeId,
                call_click_week::Column::SupplyTypeId,
                call_click_week::Column::Channel,
                call_click_week::Column::Taps,
            ])
            .to_owned();
        insert_taps
            .select_from(tap_weeks)
            .map_err(|e| Error::internal(format!("demand roll-up taps: {e}")))?;
        run(&txn, "count taps", &insert_taps).await?;
        let taps = run(
            &txn,
            "delete taps",
            &Query::delete()
                .from_table(call_click::Entity)
                .cond_where(within(call_click::Column::CreatedOn, from, to))
                .to_owned(),
        )
        .await?;

        let unmet = Expr::case(Expr::col(search_event::Column::Results).eq(0), 1).finally(0);
        let search_weeks = Query::select()
            .expr(Expr::val(week_starts_on))
            .columns([
                search_event::Column::ServiceTypeId,
                search_event::Column::SupplyTypeId,
                search_event::Column::WardCode,
            ])
            .expr(count_int())
            .expr(SimpleExpr::from(Func::sum(unmet)).cast_as(PgType::Int4))
            .column(search_event::Column::IsDemo)
            .from(search_event::Entity)
            .cond_where(within(search_event::Column::CreatedOn, from, to))
            .group_by_columns([
                search_event::Column::ServiceTypeId,
                search_event::Column::SupplyTypeId,
                search_event::Column::WardCode,
                search_event::Column::IsDemo,
            ])
            .to_owned();
        let mut insert_searches = Query::insert()
            .into_table(search_event_week::Entity)
            .columns([
                search_event_week::Column::WeekStartsOn,
                search_event_week::Column::ServiceTypeId,
                search_event_week::Column::SupplyTypeId,
                search_event_week::Column::WardCode,
                search_event_week::Column::Searches,
                search_event_week::Column::Unmet,
                search_event_week::Column::IsDemo,
            ])
            .to_owned();
        insert_searches
            .select_from(search_weeks)
            .map_err(|e| Error::internal(format!("demand roll-up searches: {e}")))?;
        run(&txn, "count searches", &insert_searches).await?;
        let searches = run(
            &txn,
            "delete searches",
            &Query::delete()
                .from_table(search_event::Entity)
                .cond_where(within(search_event::Column::CreatedOn, from, to))
                .to_owned(),
        )
        .await?;

        txn.commit().await.map_err(roll_up_error("commit"))?;
        Ok(taps + searches)
    }
}
