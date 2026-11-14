//! Provinces and wards: the bundled dataset (`data/admin_units/`, built by
//! its `build.sql`), loaded into the database once per version, and the
//! point-in-ward question, which needs the boundaries.
//!
//! A ward's boundary is a `GEOGRAPHY(MULTIPOLYGON)`: a point counts under
//! the ward that covers it (`ST_Covers`, through the GiST index, a few
//! milliseconds; on a shared edge the lower code), and one just outside
//! every ward (a coastline, a sliver between two simplified boundaries)
//! under the nearest one within the snap distance (the index's `<->`
//! ordering). `ST_Distance` to a boundary of thousands of vertices takes
//! seconds, so it is never used here.

use std::{io::Read, sync::Arc};

use async_trait::async_trait;
use flate2::read::GzDecoder;
use sea_orm::{
    sea_query::{
        extension::postgres::PgBinOper, Expr, Func, OnConflict, Order, Query, SelectStatement,
        SimpleExpr,
    },
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};

use domain::{
    models::admin_unit::{AdminUnitSync, Province, Ward},
    repositories::admin_unit_repository_trait::AdminUnitRepositoryTrait,
    Error,
};

use crate::{
    entities::{catalogue_state, province, ward},
    sql::{advisory_xact_lock, make_point, scalar, within, DbFn, Geo, PgType, QueryAs},
};

/// The dataset: `V version`, then `P` (province) and `W` (ward) lines, tab
/// separated; a ward's boundary is TWKB (precision 5) in hex.
const DATASET: &[u8] = include_bytes!("../../data/admin_units/admin_units.tsv.gz");

/// Recorded in `catalogue_state` with the version once a dataset is in.
const DATASET_KEY_PREFIX: &str = "admin_units.";

/// Two processes starting together load the dataset once.
const LOAD_LOCK_ID: i64 = 0x6167_7269_5f61_6475; // "agri_adu"

/// Wards per INSERT: 8 values each, far under Postgres' 65 535 parameters.
const WARDS_PER_STATEMENT: usize = 100;

/// The datum of every stored shape.
const SRID: i32 = 4326;

pub struct AdminUnitRepository {
    db: Arc<DatabaseConnection>,
}

impl AdminUnitRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    fn fail(step: &'static str) -> impl Fn(DbErr) -> Error {
        move |e| Error::internal(format!("admin units {step}: {e}"))
    }
}

/// `"wards"."boundary"`.
fn boundary() -> SimpleExpr {
    Expr::col((ward::Entity, Geo::Boundary)).into()
}

/// The code of the active ward `point` lies in, else of the nearest within
/// `snap_m` metres, else NULL: `COALESCE((SELECT … covers), (SELECT …
/// nearest))`; Postgres runs the second only when the first finds none.
/// The one definition of "the ward of a point".
pub(crate) fn ward_code_at(point: SimpleExpr, snap_m: f64) -> SimpleExpr {
    Func::coalesce([
        scalar(active_wards().and_where(covers(point.clone())).to_owned()),
        scalar(
            active_wards()
                .and_where(within(boundary(), point.clone(), snap_m))
                .order_by_expr(knn_distance(point), Order::Asc)
                .to_owned(),
        ),
    ])
    .into()
}

/// `SELECT code FROM wards WHERE is_active … ORDER BY code LIMIT 1`, the
/// conditions and leading order to add.
fn active_wards() -> SelectStatement {
    Query::select()
        .column((ward::Entity, ward::Column::Code))
        .from(ward::Entity)
        .and_where(Expr::col((ward::Entity, ward::Column::IsActive)).eq(true))
        .limit(1)
        .to_owned()
}

/// `ST_Covers(boundary, point)`: the point is inside or on the edge.
fn covers(point: SimpleExpr) -> SimpleExpr {
    Func::cust(DbFn::StCovers).args([boundary(), point]).into()
}

/// `boundary <-> point`: PostGIS' index-assisted distance, for ordering.
/// sea-query names the operator after pg_trgm's use of the same symbol.
fn knn_distance(point: SimpleExpr) -> SimpleExpr {
    boundary().binary(PgBinOper::SimilarityDistance, point)
}

/// `ST_Y(column::geometry)` / `ST_X(column::geometry)` of a point column.
fn coordinate(f: DbFn, table: impl sea_orm::Iden + 'static) -> SimpleExpr {
    Func::cust(f)
        .arg(SimpleExpr::from(Expr::col((table, Geo::Center))).cast_as(PgType::Geometry))
        .into()
}

/// `ST_SetSRID(ST_MakePoint(lng, lat), 4326)::geography`, from values.
fn point_value(lat: f64, lng: f64) -> SimpleExpr {
    make_point(Expr::val(lng), Expr::val(lat))
}

/// `ST_SetSRID(ST_GeomFromTWKB(decode(hex, 'hex')), 4326)::geography`.
fn boundary_value(twkb_hex: &str) -> SimpleExpr {
    let args: [SimpleExpr; 2] = [Expr::val(twkb_hex).into(), Expr::val("hex").into()];
    let bytes = Func::cust(DbFn::Decode).args(args);
    let shape = Func::cust(DbFn::StGeomFromTwkb).arg(bytes);
    SimpleExpr::from(Func::cust(DbFn::StSetSrid).args([shape.into(), Expr::val(SRID).into()]))
        .cast_as(PgType::Geography)
}

#[derive(FromQueryResult)]
struct ProvinceRow {
    code: String,
    name: String,
    full_name: String,
    unit_type: String,
    is_active: bool,
    lat: f64,
    lng: f64,
}

#[derive(FromQueryResult)]
struct WardRow {
    code: String,
    province_code: String,
    name: String,
    full_name: String,
    unit_type: String,
    is_active: bool,
    lat: f64,
    lng: f64,
}

impl From<WardRow> for Ward {
    fn from(r: WardRow) -> Self {
        Ward {
            code: r.code,
            province_code: r.province_code,
            name: r.name,
            full_name: r.full_name,
            unit_type: r.unit_type,
            lat: r.lat,
            lng: r.lng,
            is_active: r.is_active,
        }
    }
}

/// The wards without their boundaries, with their point's coordinates.
fn wards_query() -> sea_orm::Select<ward::Entity> {
    ward::Entity::find()
        .select_only()
        .columns([
            ward::Column::Code,
            ward::Column::ProvinceCode,
            ward::Column::Name,
            ward::Column::FullName,
            ward::Column::UnitType,
            ward::Column::IsActive,
        ])
        .expr_as(coordinate(DbFn::StY, ward::Entity), QueryAs::Lat)
        .expr_as(coordinate(DbFn::StX, ward::Entity), QueryAs::Lng)
        .order_by_asc(ward::Column::Code)
}

// ── The bundled file ──────────────────────────────────────────────────────────

struct ProvinceLine<'a> {
    code: &'a str,
    name: &'a str,
    full_name: &'a str,
    unit_type: &'a str,
    lat: f64,
    lng: f64,
}

struct WardLine<'a> {
    code: &'a str,
    province_code: &'a str,
    name: &'a str,
    full_name: &'a str,
    unit_type: &'a str,
    lat: f64,
    lng: f64,
    twkb_hex: &'a str,
}

struct Dataset<'a> {
    version: &'a str,
    provinces: Vec<ProvinceLine<'a>>,
    wards: Vec<WardLine<'a>>,
}

fn bad(line: usize, what: &str) -> Error {
    Error::internal(format!("admin units file, line {line}: {what}"))
}

fn number(text: &str, line: usize) -> Result<f64, Error> {
    text.parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
        .ok_or_else(|| bad(line, "not a coordinate"))
}

fn inflate() -> Result<String, Error> {
    let mut text = String::new();
    GzDecoder::new(DATASET)
        .read_to_string(&mut text)
        .map_err(|e| Error::internal(format!("admin units file: {e}")))?;
    Ok(text)
}

/// The lines of the file. The build writes no escapes (no name holds a tab
/// or a backslash), so one is a sign of a broken file.
fn parse(text: &str) -> Result<Dataset<'_>, Error> {
    let mut version = None;
    let mut provinces = Vec::new();
    let mut wards = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.contains('\\') {
            return Err(bad(n, "escaped text"));
        }
        let f: Vec<&str> = line.split('\t').collect();
        match f.as_slice() {
            ["V", v] => version = Some(*v),
            ["P", code, name, full_name, unit_type, lat, lng] => provinces.push(ProvinceLine {
                code,
                name,
                full_name,
                unit_type,
                lat: number(lat, n)?,
                lng: number(lng, n)?,
            }),
            ["W", code, province_code, name, full_name, unit_type, lat, lng, twkb_hex] => wards
                .push(WardLine {
                    code,
                    province_code,
                    name,
                    full_name,
                    unit_type,
                    lat: number(lat, n)?,
                    lng: number(lng, n)?,
                    twkb_hex,
                }),
            [""] => {}
            _ => return Err(bad(n, "unexpected fields")),
        }
    }
    let version = version.ok_or_else(|| bad(1, "no version"))?;
    if provinces.is_empty() || wards.is_empty() {
        return Err(bad(1, "no provinces or wards"));
    }
    Ok(Dataset {
        version,
        provinces,
        wards,
    })
}

async fn run<S: sea_orm::StatementBuilder>(
    txn: &DatabaseTransaction,
    step: &'static str,
    statement: &S,
) -> Result<u64, Error> {
    txn.execute(txn.get_database_backend().build(statement))
        .await
        .map(|r| r.rows_affected())
        .map_err(AdminUnitRepository::fail(step))
}

async fn load(txn: &DatabaseTransaction, data: &Dataset<'_>) -> Result<(u64, u64), Error> {
    // Everything off first; what the file lists comes back on.
    run(
        txn,
        "turn off wards",
        &Query::update()
            .table(ward::Entity)
            .value(ward::Column::IsActive, false)
            .to_owned(),
    )
    .await?;
    run(
        txn,
        "turn off provinces",
        &Query::update()
            .table(province::Entity)
            .value(province::Column::IsActive, false)
            .to_owned(),
    )
    .await?;

    let mut provinces = Query::insert()
        .into_table(province::Entity)
        .columns([
            province::Column::Code.into_iden_dyn(),
            province::Column::Name.into_iden_dyn(),
            province::Column::FullName.into_iden_dyn(),
            province::Column::UnitType.into_iden_dyn(),
            Geo::Center.into_iden_dyn(),
            province::Column::IsActive.into_iden_dyn(),
        ])
        .on_conflict(
            OnConflict::column(province::Column::Code)
                .update_columns([
                    province::Column::Name.into_iden_dyn(),
                    province::Column::FullName.into_iden_dyn(),
                    province::Column::UnitType.into_iden_dyn(),
                    Geo::Center.into_iden_dyn(),
                    province::Column::IsActive.into_iden_dyn(),
                ])
                .to_owned(),
        )
        .to_owned();
    for p in &data.provinces {
        provinces
            .values([
                Expr::val(p.code).into(),
                Expr::val(p.name).into(),
                Expr::val(p.full_name).into(),
                Expr::val(p.unit_type).into(),
                point_value(p.lat, p.lng),
                Expr::val(true).into(),
            ])
            .map_err(|e| Error::internal(format!("admin units provinces: {e}")))?;
    }
    let province_count = run(txn, "provinces", &provinces).await?;

    let mut ward_count = 0;
    for chunk in data.wards.chunks(WARDS_PER_STATEMENT) {
        let mut wards = Query::insert()
            .into_table(ward::Entity)
            .columns([
                ward::Column::Code.into_iden_dyn(),
                ward::Column::ProvinceCode.into_iden_dyn(),
                ward::Column::Name.into_iden_dyn(),
                ward::Column::FullName.into_iden_dyn(),
                ward::Column::UnitType.into_iden_dyn(),
                Geo::Center.into_iden_dyn(),
                Geo::Boundary.into_iden_dyn(),
                ward::Column::IsActive.into_iden_dyn(),
            ])
            .on_conflict(
                OnConflict::column(ward::Column::Code)
                    .update_columns([
                        ward::Column::ProvinceCode.into_iden_dyn(),
                        ward::Column::Name.into_iden_dyn(),
                        ward::Column::FullName.into_iden_dyn(),
                        ward::Column::UnitType.into_iden_dyn(),
                        Geo::Center.into_iden_dyn(),
                        Geo::Boundary.into_iden_dyn(),
                        ward::Column::IsActive.into_iden_dyn(),
                    ])
                    .to_owned(),
            )
            .to_owned();
        for w in chunk {
            wards
                .values([
                    Expr::val(w.code).into(),
                    Expr::val(w.province_code).into(),
                    Expr::val(w.name).into(),
                    Expr::val(w.full_name).into(),
                    Expr::val(w.unit_type).into(),
                    point_value(w.lat, w.lng),
                    boundary_value(w.twkb_hex),
                    Expr::val(true).into(),
                ])
                .map_err(|e| Error::internal(format!("admin units wards: {e}")))?;
        }
        ward_count += run(txn, "wards", &wards).await?;
    }
    Ok((province_count, ward_count))
}

trait IntoIdenDyn {
    fn into_iden_dyn(self) -> sea_orm::sea_query::DynIden;
}

impl<T: sea_orm::Iden + 'static> IntoIdenDyn for T {
    fn into_iden_dyn(self) -> sea_orm::sea_query::DynIden {
        sea_orm::sea_query::SeaRc::new(self)
    }
}

#[async_trait]
impl AdminUnitRepositoryTrait for AdminUnitRepository {
    async fn sync(&self) -> Result<AdminUnitSync, Error> {
        let text = inflate()?;
        let data = parse(&text)?;
        let key = format!("{DATASET_KEY_PREFIX}{}", data.version);
        let txn = self.db.begin().await.map_err(Self::fail("begin"))?;
        run(&txn, "lock", &advisory_xact_lock(LOAD_LOCK_ID)).await?;
        let done = catalogue_state::Entity::find_by_id(key.clone())
            .one(&txn)
            .await
            .map_err(Self::fail("version"))?
            .is_some();
        if done {
            txn.commit().await.map_err(Self::fail("commit"))?;
            return Ok(AdminUnitSync {
                version: data.version.to_owned(),
                ..AdminUnitSync::default()
            });
        }
        let (provinces, wards) = load(&txn, &data).await?;
        // Older versions' keys go: one row says which dataset is in.
        run(
            &txn,
            "forget older versions",
            &Query::delete()
                .from_table(catalogue_state::Entity)
                .and_where(catalogue_state::Column::Key.like(format!("{DATASET_KEY_PREFIX}%")))
                .to_owned(),
        )
        .await?;
        run(
            &txn,
            "record version",
            &Query::insert()
                .into_table(catalogue_state::Entity)
                .columns([catalogue_state::Column::Key])
                .values_panic([Expr::val(key).into()])
                .to_owned(),
        )
        .await?;
        txn.commit().await.map_err(Self::fail("commit"))?;
        Ok(AdminUnitSync {
            version: data.version.to_owned(),
            loaded: true,
            provinces,
            wards,
        })
    }

    async fn provinces(&self) -> Result<Vec<Province>, Error> {
        province::Entity::find()
            .select_only()
            .columns([
                province::Column::Code,
                province::Column::Name,
                province::Column::FullName,
                province::Column::UnitType,
                province::Column::IsActive,
            ])
            .expr_as(coordinate(DbFn::StY, province::Entity), QueryAs::Lat)
            .expr_as(coordinate(DbFn::StX, province::Entity), QueryAs::Lng)
            .order_by_asc(province::Column::Code)
            .into_model::<ProvinceRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| Province {
                        code: r.code,
                        name: r.name,
                        full_name: r.full_name,
                        unit_type: r.unit_type,
                        lat: r.lat,
                        lng: r.lng,
                        is_active: r.is_active,
                    })
                    .collect()
            })
            .map_err(Self::fail("provinces"))
    }

    async fn wards(&self) -> Result<Vec<Ward>, Error> {
        wards_query()
            .into_model::<WardRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| rows.into_iter().map(Ward::from).collect())
            .map_err(Self::fail("wards"))
    }

    async fn ward_at(&self, lat: f64, lng: f64, snap_m: f64) -> Result<Option<Ward>, Error> {
        wards_query()
            .filter(
                Expr::col((ward::Entity, ward::Column::Code))
                    .eq(ward_code_at(point_value(lat, lng), snap_m)),
            )
            .into_model::<WardRow>()
            .one(self.db.as_ref())
            .await
            .map(|row| row.map(Ward::from))
            .map_err(Self::fail("ward_at"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::sea_query::PostgresQueryBuilder;

    #[test]
    fn a_point_is_looked_up_by_cover_then_by_index_distance() {
        let sql = Query::select()
            .expr(ward_code_at(point_value(10.4, 106.1), 500.0))
            .to_string(PostgresQueryBuilder);
        assert!(sql.starts_with("SELECT COALESCE((SELECT"), "{sql}");
        assert!(sql.contains(r#"ST_Covers("wards"."boundary", "#), "{sql}");
        assert!(sql.contains(r#"ORDER BY "wards"."boundary" <-> "#), "{sql}");
        assert!(!sql.contains("ST_Distance"), "seconds per boundary: {sql}");
    }

    #[test]
    fn the_bundled_file_holds_every_province_and_ward() {
        let text = inflate().unwrap_or_default();
        let data = parse(&text);
        assert!(data.is_ok(), "{:?}", data.err());
        if let Ok(data) = data {
            assert_eq!(data.provinces.len(), 34);
            assert_eq!(data.wards.len(), 3321);
            let provinces: std::collections::HashSet<&str> =
                data.provinces.iter().map(|p| p.code).collect();
            assert!(data
                .wards
                .iter()
                .all(|w| provinces.contains(w.province_code)));
            // Every point is in Vietnam's bounding box (with its islands).
            assert!(data
                .wards
                .iter()
                .all(|w| (6.0..24.0).contains(&w.lat) && (102.0..118.0).contains(&w.lng)));
        }
    }
}
