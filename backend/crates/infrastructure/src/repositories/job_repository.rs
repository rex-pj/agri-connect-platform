//! Job posts. The geography column, the distance to a provider and the
//! "live service in the category" rule meet in one statement each, so these
//! are SQL statements with `$n` placeholders (as the inquiry repository's
//! owner queries), not builder queries.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, FromQueryResult, Statement, Value};
use tracing::error;

use domain::{
    models::job::{JobForNotice, JobPost, JobRecipient, NewJobPost},
    repositories::job_repository_trait::JobRepositoryTrait,
    Error,
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

    fn statement(&self, sql: &str, values: Vec<Value>) -> Statement {
        Statement::from_sql_and_values(self.db.get_database_backend(), sql, values)
    }
}

/// A live service of business `b` in the job `j`'s category, in a shown
/// category: what makes a provider one to tell.
const OFFERS_THE_JOB: &str = "EXISTS (SELECT 1 FROM services s \
     JOIN categories c ON c.id = s.category_id AND c.is_active \
     WHERE s.business_id = b.id AND s.category_id = j.category_id \
       AND s.is_active AND s.deleted_at IS NULL)";

/// The columns of a `JobPost` row, from `job_posts j`, `categories c` and
/// `locations l` (left-joined).
const JOB_COLUMNS: &str = "j.id, j.category_id, c.name AS category, l.label AS area, \
     j.description, j.requester_name, j.requester_phone, j.status, j.created_on";

#[derive(FromQueryResult)]
struct IdRow {
    id: i32,
}

#[derive(FromQueryResult)]
struct CountRow {
    n: i64,
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
        IdRow::find_by_statement(self.statement(
            "INSERT INTO job_posts \
                 (category_id, location_id, location, description, requester_name, requester_phone) \
             VALUES ($1, $2, ST_GeographyFromText($3), $4, $5, $6) RETURNING id",
            vec![
                job.category_id.into(),
                job.location_id.into(),
                job.point_wkt.clone().into(),
                job.description.clone().into(),
                job.requester_name.clone().into(),
                job.requester_phone.clone().into(),
            ],
        ))
        .one(self.db.as_ref())
        .await
        .map_err(Self::fail("create"))?
        .map(|r| r.id)
        .ok_or_else(|| Error::internal("job insert returned no id"))
    }

    async fn count_open_by_phone(&self, phone: &str, since: DateTime<Utc>) -> Result<u64, Error> {
        let row = CountRow::find_by_statement(self.statement(
            "SELECT COUNT(*) AS n FROM job_posts \
             WHERE requester_phone = $1 AND status = 'open' AND created_on >= $2",
            vec![phone.into(), since.into()],
        ))
        .one(self.db.as_ref())
        .await
        .map_err(Self::fail("count_open_by_phone"))?;
        Ok(row.map_or(0, |r| u64::try_from(r.n).unwrap_or(0)))
    }

    async fn find_for_notice(
        &self,
        id: i32,
        radius_m: f64,
        limit: i64,
    ) -> Result<Option<JobForNotice>, Error> {
        let db = self.db.as_ref();
        let Some(job) = JobRow::find_by_statement(self.statement(
            &format!(
                "SELECT {JOB_COLUMNS}, NULL::float8 AS distance_m FROM job_posts j \
                 JOIN categories c ON c.id = j.category_id \
                 LEFT JOIN locations l ON l.id = j.location_id WHERE j.id = $1"
            ),
            vec![id.into()],
        ))
        .one(db)
        .await
        .map_err(Self::fail("find_for_notice"))?
        else {
            return Ok(None);
        };
        // The managing account's address, if it is live, else the business's.
        let recipients = RecipientRow::find_by_statement(self.statement(
            &format!(
                "SELECT b.name AS business_name, \
                        COALESCE(u.email, NULLIF(TRIM(b.email), '')) AS email \
                 FROM job_posts j \
                 JOIN businesses b ON b.is_active AND b.deleted_at IS NULL AND NOT b.is_demo \
                      AND b.location IS NOT NULL \
                      AND ST_DWithin(b.location, j.location, $2) \
                 LEFT JOIN users u ON u.id = b.owner_id AND u.is_active AND u.deleted_at IS NULL \
                 WHERE j.id = $1 AND {OFFERS_THE_JOB} \
                   AND COALESCE(u.email, NULLIF(TRIM(b.email), '')) IS NOT NULL \
                 ORDER BY ST_Distance(b.location, j.location), b.id LIMIT $3"
            ),
            vec![id.into(), radius_m.into(), limit.into()],
        ))
        .all(db)
        .await
        .map_err(Self::fail("recipients"))?;
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
    ) -> Result<Vec<JobPost>, Error> {
        JobRow::find_by_statement(self.statement(
            &format!(
                "SELECT {JOB_COLUMNS}, ST_Distance(j.location, b.location) AS distance_m \
                 FROM job_posts j \
                 JOIN businesses b ON b.owner_id = $1 AND b.is_active AND b.deleted_at IS NULL \
                      AND b.location IS NOT NULL \
                 JOIN categories c ON c.id = j.category_id \
                 LEFT JOIN locations l ON l.id = j.location_id \
                 WHERE j.status = 'open' AND j.created_on >= $2 \
                   AND ST_DWithin(j.location, b.location, $3) AND {OFFERS_THE_JOB} \
                 ORDER BY j.created_on DESC, j.id DESC LIMIT $4"
            ),
            vec![
                owner_user.into(),
                since.into(),
                radius_m.into(),
                limit.into(),
            ],
        ))
        .all(self.db.as_ref())
        .await
        .map(|rows| rows.into_iter().map(JobPost::from).collect())
        .map_err(Self::fail("open_near_owner"))
    }

    async fn list_admin(&self, limit: i64, offset: i64) -> Result<(Vec<JobPost>, i64), Error> {
        let db = self.db.as_ref();
        let total = CountRow::find_by_statement(
            self.statement("SELECT COUNT(*) AS n FROM job_posts", vec![]),
        )
        .one(db)
        .await
        .map_err(Self::fail("count"))?
        .map_or(0, |r| r.n);
        let rows = JobRow::find_by_statement(self.statement(
            &format!(
                "SELECT {JOB_COLUMNS}, NULL::float8 AS distance_m FROM job_posts j \
                 JOIN categories c ON c.id = j.category_id \
                 LEFT JOIN locations l ON l.id = j.location_id \
                 ORDER BY j.created_on DESC, j.id DESC LIMIT $1 OFFSET $2"
            ),
            vec![limit.into(), offset.into()],
        ))
        .all(db)
        .await
        .map_err(Self::fail("list_admin"))?;
        Ok((rows.into_iter().map(JobPost::from).collect(), total))
    }

    async fn set_status(&self, id: i32, status: &str) -> Result<bool, Error> {
        self.db
            .execute(self.statement(
                "UPDATE job_posts SET status = $2 WHERE id = $1",
                vec![id.into(), status.into()],
            ))
            .await
            .map(|r| r.rows_affected() > 0)
            .map_err(Self::fail("set_status"))
    }
}
