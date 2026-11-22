//! Job posts. The geography column, the distance to a provider and the
//! "live service in the category" rule meet in one statement each, built
//! with the query builder (`OFFERS_THE_JOB` is `offers_the_job`).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::{Asterisk, Expr, Func, IntoIden, Order, Query, SelectStatement, SimpleExpr},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, FromQueryResult,
    PaginatorTrait, QueryFilter, StatementBuilder,
};
use tracing::error;

use domain::{
    constants::JOB_STATUS_OPEN,
    models::admin_list::{AdminFilter, AdminSort},
    models::job::{JobForNotice, JobPost, JobRecipient, NewJobPost},
    repositories::job_repository_trait::JobRepositoryTrait,
    Error,
};

use crate::{
    entities::{business, job_post, location, service, service_type as category, user},
    search::admin_job_search,
    sql::{
        business_location, distance, geography_from_text, live_owner_on, notice_email, null_float,
        within, Geo, QueryAs,
    },
};

pub struct JobRepository {
    db: Arc<DatabaseConnection>,
}

impl JobRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    fn fail(what: &'static str) -> impl Fn(DbErr) -> Error {
        move |e| {
            error!("job {what} error: {e}");
            Error::internal("Failed to load or save the job post.")
        }
    }

    async fn rows<T: FromQueryResult, S: StatementBuilder>(
        &self,
        query: &S,
        what: &'static str,
    ) -> Result<Vec<T>, Error> {
        let db = self.db.as_ref();
        T::find_by_statement(db.get_database_backend().build(query))
            .all(db)
            .await
            .map_err(Self::fail(what))
    }
}

/// `"job_posts"."location"`.
fn job_location() -> SimpleExpr {
    Expr::col((job_post::Entity, Geo::Location)).into()
}

/// A live service of the business in the job's category, in a shown
/// category: what makes a provider one to tell (OFFERS_THE_JOB).
fn offers_the_job() -> SimpleExpr {
    Expr::exists(
        Query::select()
            .expr(Expr::val(1))
            .from(service::Entity)
            .inner_join(
                category::Entity,
                Expr::col((category::Entity, category::Column::Id))
                    .equals((service::Entity, service::Column::ServiceTypeId))
                    .and(Expr::col((category::Entity, category::Column::IsActive)).eq(true)),
            )
            .and_where(
                Expr::col((service::Entity, service::Column::BusinessId))
                    .equals((business::Entity, business::Column::Id)),
            )
            .and_where(
                Expr::col((service::Entity, service::Column::ServiceTypeId))
                    .equals((job_post::Entity, job_post::Column::ServiceTypeId)),
            )
            .and_where(Expr::col((service::Entity, service::Column::IsActive)).eq(true))
            .and_where(Expr::col((service::Entity, service::Column::DeletedAt)).is_null())
            .to_owned(),
    )
}

/// The columns of a `JobRow`, from `job_posts` with its category and its
/// area (left-joined); `distance_m` as given.
fn job_select(distance_m: SimpleExpr) -> SelectStatement {
    Query::select()
        .column((job_post::Entity, job_post::Column::Id))
        .expr_as(
            Expr::col((job_post::Entity, job_post::Column::ServiceTypeId)),
            QueryAs::CategoryId,
        )
        .expr_as(
            Expr::col((category::Entity, category::Column::Name)),
            QueryAs::Category,
        )
        .expr_as(
            Expr::col((location::Entity, location::Column::Label)),
            QueryAs::Area,
        )
        .column((job_post::Entity, job_post::Column::Description))
        .column((job_post::Entity, job_post::Column::RequesterName))
        .column((job_post::Entity, job_post::Column::RequesterPhone))
        .column((job_post::Entity, job_post::Column::Status))
        .column((job_post::Entity, job_post::Column::CreatedOn))
        .expr_as(distance_m, QueryAs::DistanceM)
        .from(job_post::Entity)
        .inner_join(
            category::Entity,
            Expr::col((category::Entity, category::Column::Id))
                .equals((job_post::Entity, job_post::Column::ServiceTypeId)),
        )
        .left_join(
            location::Entity,
            Expr::col((location::Entity, location::Column::Id))
                .equals((job_post::Entity, job_post::Column::LocationId)),
        )
        .to_owned()
}

/// Newest first, id as the tiebreaker.
fn newest_first(query: &mut SelectStatement) -> &mut SelectStatement {
    query
        .order_by((job_post::Entity, job_post::Column::CreatedOn), Order::Desc)
        .order_by((job_post::Entity, job_post::Column::Id), Order::Desc)
}

/// The live business: switched on and not deleted.
fn live_business() -> SimpleExpr {
    Expr::col((business::Entity, business::Column::IsActive))
        .eq(true)
        .and(Expr::col((business::Entity, business::Column::DeletedAt)).is_null())
        .and(Expr::expr(business_location()).is_not_null())
}

/// The open posts since `since` within `radius_m` of the business the
/// account `owner_user` manages, in the categories of its live services:
/// what its list pages through and what its count counts, the one
/// definition (`OFFERS_THE_JOB`).
fn near_owner(
    query: &mut SelectStatement,
    owner_user: i32,
    since: DateTime<Utc>,
    radius_m: f64,
) -> &mut SelectStatement {
    query
        .inner_join(
            business::Entity,
            Expr::col((business::Entity, business::Column::OwnerId))
                .eq(owner_user)
                .and(live_business()),
        )
        .and_where(Expr::col((job_post::Entity, job_post::Column::Status)).eq(JOB_STATUS_OPEN))
        .and_where(Expr::col((job_post::Entity, job_post::Column::CreatedOn)).gte(since))
        .and_where(within(job_location(), business_location(), radius_m))
        .and_where(offers_the_job())
        // A real business sees real posts, a demo one demo posts.
        .and_where(
            Expr::col((job_post::Entity, job_post::Column::IsDemo))
                .equals((business::Entity, business::Column::IsDemo)),
        )
}

#[derive(FromQueryResult)]
struct IdRow {
    id: i32,
}

#[derive(FromQueryResult)]
struct TotalRow {
    total: i64,
}

#[derive(FromQueryResult)]
struct JobRow {
    id: i32,
    category_id: i32,
    category: String,
    area: Option<String>,
    description: String,
    requester_name: String,
    requester_phone: String,
    status: String,
    created_on: DateTime<Utc>,
    distance_m: Option<f64>,
}

impl From<JobRow> for JobPost {
    fn from(r: JobRow) -> Self {
        Self {
            id: r.id,
            category_id: r.category_id,
            category: r.category,
            area: r.area,
            description: r.description,
            requester_name: r.requester_name,
            requester_phone: r.requester_phone,
            status: r.status,
            created_on: r.created_on,
            distance_m: r.distance_m,
        }
    }
}

#[derive(FromQueryResult)]
struct RecipientRow {
    business_name: String,
    email: String,
}

#[async_trait]
impl JobRepositoryTrait for JobRepository {
    async fn create(&self, job: &NewJobPost) -> Result<i32, Error> {
        let insert = Query::insert()
            .into_table(job_post::Entity)
            .columns([
                job_post::Column::ServiceTypeId.into_iden(),
                job_post::Column::LocationId.into_iden(),
                Geo::Location.into_iden(),
                job_post::Column::Description.into_iden(),
                job_post::Column::RequesterName.into_iden(),
                job_post::Column::RequesterPhone.into_iden(),
            ])
            .values([
                Expr::val(job.category_id).into(),
                Expr::val(job.location_id).into(),
                geography_from_text(&job.point_wkt),
                Expr::val(job.description.clone()).into(),
                Expr::val(job.requester_name.clone()).into(),
                Expr::val(job.requester_phone.clone()).into(),
            ])
            .map_err(|e| Error::internal(format!("job insert: {e}")))?
            .returning_col(job_post::Column::Id)
            .to_owned();
        self.rows::<IdRow, _>(&insert, "create")
            .await?
            .first()
            .map(|r| r.id)
            .ok_or_else(|| Error::internal("job insert returned no id"))
    }

    async fn count_open_by_phone(&self, phone: &str, since: DateTime<Utc>) -> Result<u64, Error> {
        job_post::Entity::find()
            .filter(job_post::Column::RequesterPhone.eq(phone))
            // Demo posts never hold a farmer back.
            .filter(job_post::Column::IsDemo.eq(false))
            .filter(job_post::Column::Status.eq(JOB_STATUS_OPEN))
            .filter(job_post::Column::CreatedOn.gte(since))
            .count(self.db.as_ref())
            .await
            .map_err(Self::fail("count_open_by_phone"))
    }

    async fn find_for_notice(
        &self,
        id: i32,
        radius_m: f64,
        limit: i64,
    ) -> Result<Option<JobForNotice>, Error> {
        let Some(job) = self
            .rows::<JobRow, _>(
                &*job_select(null_float())
                    .and_where(Expr::col((job_post::Entity, job_post::Column::Id)).eq(id)),
                "find_for_notice",
            )
            .await?
            .into_iter()
            .next()
        else {
            return Ok(None);
        };
        // The managing account's address, if it is live, else the business's.
        let recipients = Query::select()
            .expr_as(
                Expr::col((business::Entity, business::Column::Name)),
                QueryAs::BusinessName,
            )
            .expr_as(notice_email(), QueryAs::Email)
            .from(job_post::Entity)
            .inner_join(
                business::Entity,
                live_business()
                    .and(Expr::col((business::Entity, business::Column::IsDemo)).eq(false))
                    .and(within(business_location(), job_location(), radius_m)),
            )
            .left_join(user::Entity, live_owner_on())
            .and_where(Expr::col((job_post::Entity, job_post::Column::Id)).eq(id))
            .and_where(offers_the_job())
            .and_where(Expr::expr(notice_email()).is_not_null())
            .order_by_expr(distance(business_location(), job_location()), Order::Asc)
            .order_by((business::Entity, business::Column::Id), Order::Asc)
            .limit(u64::try_from(limit).unwrap_or(0))
            .to_owned();
        let recipients = self
            .rows::<RecipientRow, _>(&recipients, "recipients")
            .await?;
        Ok(Some(JobForNotice {
            category: job.category,
            area: job.area,
            description: job.description,
            requester_name: job.requester_name,
            requester_phone: job.requester_phone,
            recipients: recipients
                .into_iter()
                .map(|r| JobRecipient {
                    business_name: r.business_name,
                    email: r.email,
                })
                .collect(),
        }))
    }

    async fn open_near_owner(
        &self,
        owner_user: i32,
        since: DateTime<Utc>,
        radius_m: f64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<JobPost>, Error> {
        let mut query = job_select(distance(job_location(), business_location()));
        near_owner(&mut query, owner_user, since, radius_m)
            .limit(u64::try_from(limit).unwrap_or(0))
            .offset(u64::try_from(offset).unwrap_or(0));
        self.rows::<JobRow, _>(&*newest_first(&mut query), "open_near_owner")
            .await
            .map(|rows| rows.into_iter().map(JobPost::from).collect())
    }

    async fn count_open_near_owner(
        &self,
        owner_user: i32,
        since: DateTime<Utc>,
        radius_m: f64,
    ) -> Result<i64, Error> {
        let mut query = Query::select()
            .expr_as(Func::count(Expr::col(Asterisk)), QueryAs::Total)
            .from(job_post::Entity)
            .to_owned();
        near_owner(&mut query, owner_user, since, radius_m);
        let rows = self
            .rows::<TotalRow, _>(&query, "count_open_near_owner")
            .await?;
        Ok(rows.first().map_or(0, |r| r.total))
    }

    async fn list_admin(
        &self,
        filter: &AdminFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<JobPost>, Error> {
        let mut query = job_select(null_float());
        query
            .cond_where(admin_condition(filter))
            .limit(u64::try_from(limit).unwrap_or(0))
            .offset(u64::try_from(offset).unwrap_or(0));
        let ordered = match filter.sort {
            Some(AdminSort::Oldest) => query
                .order_by((job_post::Entity, job_post::Column::CreatedOn), Order::Asc)
                .order_by((job_post::Entity, job_post::Column::Id), Order::Asc),
            Some(AdminSort::Name) => query
                .order_by(
                    (job_post::Entity, job_post::Column::RequesterName),
                    Order::Asc,
                )
                .order_by((job_post::Entity, job_post::Column::Id), Order::Asc),
            Some(AdminSort::Newest) | None => newest_first(&mut query),
        };
        let rows = self.rows::<JobRow, _>(&*ordered, "list_admin").await?;
        Ok(rows.into_iter().map(JobPost::from).collect())
    }

    async fn count_admin(&self, filter: &AdminFilter) -> Result<i64, Error> {
        job_post::Entity::find()
            .filter(admin_condition(filter))
            .count(self.db.as_ref())
            .await
            .map(|total| i64::try_from(total).unwrap_or(i64::MAX))
            .map_err(Self::fail("count"))
    }

    async fn set_status(&self, id: i32, status: &str) -> Result<bool, Error> {
        job_post::Entity::update_many()
            .col_expr(job_post::Column::Status, Expr::value(status.to_owned()))
            .filter(job_post::Column::Id.eq(id))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected > 0)
            .map_err(Self::fail("set_status"))
    }

    async fn delete_older_than(&self, before: DateTime<Utc>) -> Result<u64, Error> {
        job_post::Entity::delete_many()
            .filter(job_post::Column::CreatedOn.lt(before))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected)
            .map_err(Self::fail("delete_older_than"))
    }
}

/// The admin list's criteria, shared by the list and its count.
fn admin_condition(filter: &AdminFilter) -> sea_orm::Condition {
    admin_condition_base(filter).add(crate::admin_list::created_between(
        (
            crate::entities::job_post::Entity,
            crate::entities::job_post::Column::CreatedOn,
        ),
        filter,
    ))
}

fn admin_condition_base(filter: &AdminFilter) -> sea_orm::Condition {
    // The search is an OR of its columns: wrapped, so every other
    // criterion narrows it rather than joining the OR.
    let cond = sea_orm::Condition::all().add(admin_job_search(filter.search()));
    let cond = match filter.category_id {
        Some(id) => cond.add(Expr::col((job_post::Entity, job_post::Column::ServiceTypeId)).eq(id)),
        None => cond,
    };
    let cond = match crate::admin_list::stale_cutoff(filter) {
        Some(cutoff) => cond
            .add(
                Expr::col((job_post::Entity, job_post::Column::Status))
                    .eq(domain::constants::JOB_STATUS_OPEN),
            )
            .add(Expr::col((job_post::Entity, job_post::Column::CreatedOn)).lt(cutoff)),
        None => cond,
    };
    let cond = match filter.location_id {
        Some(id) => cond.add(Expr::col((job_post::Entity, job_post::Column::LocationId)).eq(id)),
        None => cond,
    };
    match filter.state {
        Some(status) => {
            cond.add(Expr::col((job_post::Entity, job_post::Column::Status)).eq(status))
        }
        None => cond,
    }
}
