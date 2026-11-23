//! Demo data for trying the platform out, loaded only on request: a starting
//! catalogue (commodities, service and supply types, area presets, crops)
//! and listings in it.
//!
//! The rows are data (`demo/data.json`), written through the query builder,
//! every commodity, type, area, crop and business flagged `is_demo`.
//! Services and products name their business and type slug, a supply its
//! commodity; a slug or name an admin changed makes its demo rows silently
//! drop out, and the returned counts show it. Removing keeps a demo
//! commodity, type, area or crop that real data uses by then (`prune`): it
//! is real from then on.

use std::{collections::HashMap, sync::Arc};

use application::{constants::ADMIN_UNIT_SNAP_M, validation::point_wkt};
use async_trait::async_trait;
use chrono::Utc;
use domain::{
    constants::{ROLE_PARTNER, ROLE_PROVIDER},
    models::catalogue::{DemoCounts, DemoRemoval, DemoReport, DemoVolume},
    repositories::{
        admin_unit_repository_trait::AdminUnitRepositoryTrait,
        demo_data_repository_trait::DemoDataRepositoryTrait,
    },
    Error, Result,
};
use sea_orm::{
    sea_query::{Expr, IntoIden, LockType, OnConflict, Query, SelectStatement, SimpleExpr},
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr,
    EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter, QuerySelect, QueryTrait, Set,
    TransactionTrait,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    demo_activity::{self, insert_chunks, Activity, DemoAccount, Refs, CHUNK},
    demo_volume::{self, vocabulary, Vocabulary, WardPlace},
    entities::{
        alert, alert_kind, business, call_click, commodity, crop, crop_price, inquiry, job_post,
        location, partner, partner_contract, partner_contract_service_type,
        partner_contract_supply_type, product, review, role, search_event, search_event_week,
        sector, service, service_type, supply_type, user,
    },
    repositories::admin_unit_repository::{ward_code_at, AdminUnitRepository},
    sql::{geography_from_text, null, Geo},
};

const DEMO_DATA: &str = include_str!("demo/data.json");

#[derive(Deserialize)]
pub(crate) struct DemoData {
    pub(crate) sectors: Vec<DemoCoded>,
    pub(crate) alert_kinds: Vec<DemoCoded>,
    pub(crate) commodities: Vec<DemoCommodity>,
    pub(crate) service_types: Vec<DemoType>,
    pub(crate) supply_types: Vec<DemoType>,
    pub(crate) locations: Vec<DemoLocation>,
    pub(crate) crops: Vec<DemoCrop>,
    pub(crate) businesses: Vec<DemoBusiness>,
    pub(crate) services: Vec<DemoService>,
    pub(crate) products: Vec<DemoProduct>,
    #[serde(flatten)]
    pub(crate) activity: Activity,
}

/// A sector the platform serves or a kind of alert; found again by code.
#[derive(Deserialize, Clone)]
pub(crate) struct DemoCoded {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) display_order: i16,
}

/// What farmers raise; found again by name.
#[derive(Deserialize, Clone)]
pub(crate) struct DemoCommodity {
    pub(crate) name: String,
    pub(crate) sector: String,
    pub(crate) icon: String,
    pub(crate) display_order: i16,
}

/// A service or supply type; found again by slug.
#[derive(Deserialize, Clone)]
pub(crate) struct DemoType {
    pub(crate) name: String,
    pub(crate) slug: String,
    pub(crate) icon: String,
    pub(crate) color: String,
    pub(crate) description: String,
    pub(crate) display_order: i16,
    /// The sector it serves; absent: every sector.
    #[serde(default)]
    pub(crate) sector: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct DemoLocation {
    pub(crate) label: String,
    pub(crate) lat: f64,
    pub(crate) lng: f64,
    pub(crate) display_order: i32,
}

#[derive(Deserialize)]
pub(crate) struct DemoCrop {
    pub(crate) name: String,
    /// The name of its commodity (one of the file's, or one already there).
    pub(crate) commodity: String,
    pub(crate) unit: String,
    pub(crate) display_order: i32,
}

#[derive(Deserialize)]
pub(crate) struct DemoBusiness {
    pub(crate) name: String,
    pub(crate) phone: String,
    pub(crate) description: String,
    pub(crate) address: String,
    pub(crate) lat: Option<f64>,
    pub(crate) lng: Option<f64>,
    pub(crate) is_verified: bool,
    /// Its managing account (role provider); loaded only at volume, where
    /// the accounts have a password.
    #[serde(default)]
    pub(crate) account: Option<DemoAccount>,
}

#[derive(Deserialize)]
pub(crate) struct DemoService {
    pub(crate) business: String,
    /// A service type's slug.
    pub(crate) category_slug: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) price_note: String,
}

#[derive(Deserialize)]
pub(crate) struct DemoProduct {
    pub(crate) business: String,
    /// A supply type's slug.
    pub(crate) category_slug: String,
    /// The commodity it is for; absent: every one.
    #[serde(default)]
    pub(crate) commodity: Option<String>,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) price: i64,
    pub(crate) unit: String,
}

/// What loading the demo catalogue inserted, by kind.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CatalogueCounts {
    pub sectors: u64,
    pub alert_kinds: u64,
    pub commodities: u64,
    pub categories: u64,
    pub locations: u64,
    pub crops: u64,
}

pub struct DemoDataRepository {
    db: Arc<DatabaseConnection>,
}

impl DemoDataRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Only the demo catalogue (commodities, types, areas, crops), without
    /// listings: what tests that need a type or an area start from.
    pub async fn load_catalogue(&self) -> Result<CatalogueCounts> {
        let data = demo_data()?;
        let txn = self.db.begin().await.map_err(db_error("begin"))?;
        let inserted = insert_catalogue(&txn, &data).await?;
        txn.commit().await.map_err(db_error("commit"))?;
        Ok(inserted)
    }

    /// The wards of the vocabulary's provinces, with their province's name:
    /// where the demo data at volume places its shops and presets.
    async fn places(&self, vocab: &Vocabulary) -> Result<Vec<WardPlace>> {
        let units = AdminUnitRepository::new(self.db.clone());
        let provinces: HashMap<String, String> = units
            .provinces()
            .await?
            .into_iter()
            .map(|p| (p.code, p.name))
            .collect();
        Ok(units
            .wards()
            .await?
            .into_iter()
            .filter(|w| w.is_active && vocab.provinces.iter().any(|p| p.code == w.province_code))
            .filter_map(|w| {
                Some(WardPlace {
                    province: provinces.get(&w.province_code)?.clone(),
                    province_code: w.province_code,
                    name: w.name,
                    full_name: w.full_name,
                    lat: w.lat,
                    lng: w.lng,
                })
            })
            .collect())
    }

    /// Writes `data` in one transaction: all or nothing, since half-loaded
    /// demo data would make `count` say "loaded" while listings are
    /// missing. The accounts it names are written only with a password.
    async fn load_data(&self, data: DemoData, password_hash: Option<&str>) -> Result<DemoReport> {
        let txn = self.db.begin().await.map_err(db_error("begin"))?;
        let catalogue = insert_catalogue(&txn, &data).await?;
        let (accounts, accounts_n) = match password_hash {
            Some(hash) => insert_accounts(&txn, &data, hash).await?,
            None => (HashMap::new(), 0),
        };
        let businesses = insert_businesses(&txn, &data.businesses, &accounts).await?;
        let (by_name, refs) = references(&txn, accounts).await?;
        let commodities = commodity_ids(&txn).await?;

        let services: Vec<service::ActiveModel> = data
            .services
            .iter()
            .flat_map(|s| {
                resolve(&by_name, &refs.service_types, &s.business, &s.category_slug).map(
                    move |(business_id, type_id)| service::ActiveModel {
                        business_id: Set(business_id),
                        service_type_id: Set(type_id),
                        name: Set(s.name.clone()),
                        description: Set(Some(s.description.clone())),
                        price_note: Set(Some(s.price_note.clone())),
                        ..Default::default()
                    },
                )
            })
            .collect();
        let products: Vec<product::ActiveModel> = data
            .products
            .iter()
            .flat_map(|p| {
                let commodity_id = p
                    .commodity
                    .as_ref()
                    .and_then(|name| commodities.get(name).copied());
                resolve(&by_name, &refs.supply_types, &p.business, &p.category_slug).map(
                    move |(business_id, type_id)| product::ActiveModel {
                        business_id: Set(business_id),
                        supply_type_id: Set(type_id),
                        commodity_id: Set(commodity_id),
                        name: Set(p.name.clone()),
                        description: Set(Some(p.description.clone())),
                        price: Set(Some(p.price)),
                        unit: Set(Some(p.unit.clone())),
                        ..Default::default()
                    },
                )
            })
            .collect();
        let services = insert_chunks(&txn, services, "services").await?;
        let products = insert_chunks(&txn, products, "products").await?;
        // What happened around the listings: leads, reviews, searches,
        // prices, job posts, partners.
        let activity = demo_activity::insert(&txn, &data.activity, &refs).await?;
        txn.commit().await.map_err(db_error("commit"))?;
        Ok(DemoReport {
            sectors: catalogue.sectors,
            alert_kinds: catalogue.alert_kinds,
            commodities: catalogue.commodities,
            categories: catalogue.categories,
            locations: catalogue.locations,
            crops: catalogue.crops,
            businesses,
            services,
            products,
            accounts: accounts_n,
            ..activity
        })
    }
}

pub(crate) fn demo_data() -> Result<DemoData> {
    serde_json::from_str(DEMO_DATA).map_err(|e| Error::internal(format!("demo data file: {e}")))
}

fn db_error(step: &'static str) -> impl Fn(DbErr) -> Error {
    move |e| Error::internal(format!("demo data {step}: {e}"))
}

/// `businesses.is_demo`: the rows this repository owns.
fn is_demo() -> SimpleExpr {
    business::Column::IsDemo.eq(true)
}

/// `SELECT id FROM businesses WHERE is_demo`: what a demo service or
/// product belongs to.
pub(crate) fn demo_business_ids() -> SelectStatement {
    Query::select()
        .column(business::Column::Id)
        .from(business::Entity)
        .and_where(is_demo())
        .to_owned()
}

/// Rows of `E` that belong to a demo business (`business` is its
/// `business_id` column).
async fn demo_rows<E: EntityTrait>(
    db: &DatabaseConnection,
    business: E::Column,
    step: &'static str,
) -> Result<u64>
where
    E::Model: Sync,
{
    E::find()
        .filter(business.in_subquery(demo_business_ids()))
        .count(db)
        .await
        .map_err(db_error(step))
}

/// The photos of the rows of `E` that belong to a demo business, read
/// `FOR UPDATE OF` that table only (the business rows stay unlocked), so no
/// photo can be added unseen before the cascade deletes the rows.
async fn demo_photos<E: EntityTrait>(
    txn: &DatabaseTransaction,
    business: E::Column,
    image: E::Column,
    step: &'static str,
) -> Result<Vec<Uuid>> {
    let mut photos = E::find()
        .select_only()
        .column(image)
        .filter(business.in_subquery(demo_business_ids()))
        .filter(image.is_not_null());
    QueryTrait::query(&mut photos).lock_with_tables(LockType::Update, [E::default()]);
    photos
        .into_tuple::<Uuid>()
        .all(txn)
        .await
        .map_err(db_error(step))
}

/// Inserts `rows` unless one with the same `key` is there (a demo row loaded
/// before, or a real one of that name, which the demo listings then use);
/// the number inserted.
async fn insert_missing<A>(
    txn: &DatabaseTransaction,
    rows: Vec<A>,
    key: <A::Entity as EntityTrait>::Column,
    step: &'static str,
) -> Result<u64>
where
    A: ActiveModelTrait + Send,
    <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
{
    if rows.is_empty() {
        return Ok(0);
    }
    <A::Entity as EntityTrait>::insert_many(rows)
        .on_conflict(OnConflict::column(key).do_nothing().to_owned())
        .exec_without_returning(txn)
        .await
        .map_err(db_error(step))
}

/// The demo types of one kind as rows of its table.
macro_rules! type_rows {
    ($table:ident, $types:expr) => {
        $types
            .iter()
            .map(|c| $table::ActiveModel {
                name: Set(c.name.clone()),
                slug: Set(c.slug.clone()),
                icon: Set(Some(c.icon.clone())),
                color: Set(Some(c.color.clone())),
                description: Set(Some(c.description.clone())),
                display_order: Set(c.display_order),
                sector: Set(c.sector.clone()),
                is_demo: Set(true),
                ..Default::default()
            })
            .collect::<Vec<_>>()
    };
}

/// Commodity ids by name.
async fn commodity_ids(txn: &DatabaseTransaction) -> Result<HashMap<String, i32>> {
    Ok(commodity::Entity::find()
        .select_only()
        .column(commodity::Column::Name)
        .column(commodity::Column::Id)
        .into_tuple::<(String, i32)>()
        .all(txn)
        .await
        .map_err(db_error("commodities read"))?
        .into_iter()
        .collect())
}

/// A coded list's demo entries as rows of its table (`sectors`, `alert_kinds`).
macro_rules! coded_rows {
    ($table:ident, $entries:expr) => {
        $entries
            .iter()
            .map(|s| $table::ActiveModel {
                code: Set(s.code.clone()),
                name: Set(s.name.clone()),
                icon: Set(Some(s.icon.clone())),
                display_order: Set(s.display_order),
                is_demo: Set(true),
                ..Default::default()
            })
            .collect::<Vec<_>>()
    };
}

/// Inserts the demo catalogue (commodities, types, areas, crops), every row
/// flagged `is_demo`: what the demo listings, prices and job posts refer to.
async fn insert_catalogue(txn: &DatabaseTransaction, data: &DemoData) -> Result<CatalogueCounts> {
    let sectors = insert_missing(
        txn,
        coded_rows!(sector, data.sectors),
        sector::Column::Code,
        "sectors",
    )
    .await?;
    let alert_kinds = insert_missing(
        txn,
        coded_rows!(alert_kind, data.alert_kinds),
        alert_kind::Column::Code,
        "alert kinds",
    )
    .await?;
    let commodities = data
        .commodities
        .iter()
        .map(|c| commodity::ActiveModel {
            name: Set(c.name.clone()),
            sector: Set(c.sector.clone()),
            icon: Set(Some(c.icon.clone())),
            display_order: Set(c.display_order),
            is_demo: Set(true),
            ..Default::default()
        })
        .collect();
    let commodities =
        insert_missing(txn, commodities, commodity::Column::Name, "commodities").await?;
    let services = insert_missing(
        txn,
        type_rows!(service_type, data.service_types),
        service_type::Column::Slug,
        "service types",
    )
    .await?;
    let supplies = insert_missing(
        txn,
        type_rows!(supply_type, data.supply_types),
        supply_type::Column::Slug,
        "supply types",
    )
    .await?;
    let locations = data
        .locations
        .iter()
        .map(|l| location::ActiveModel {
            label: Set(l.label.clone()),
            lat: Set(l.lat),
            lng: Set(l.lng),
            display_order: Set(l.display_order),
            is_demo: Set(true),
            ..Default::default()
        })
        .collect();
    let locations = insert_missing(txn, locations, location::Column::Label, "locations").await?;
    // Each crop of its commodity, by name (the file's, or one already there).
    let by_name = commodity_ids(txn).await?;
    let crops = data
        .crops
        .iter()
        .filter_map(|c| {
            Some(crop::ActiveModel {
                name: Set(c.name.clone()),
                commodity_id: Set(*by_name.get(&c.commodity)?),
                unit: Set(c.unit.clone()),
                display_order: Set(c.display_order),
                is_demo: Set(true),
                ..Default::default()
            })
        })
        .collect();
    Ok(CatalogueCounts {
        sectors,
        alert_kinds,
        commodities,
        categories: services + supplies,
        locations,
        crops: insert_missing(txn, crops, crop::Column::Name, "crops").await?,
    })
}

/// `SELECT column FROM table WHERE column IS NOT NULL`: the ids a table
/// refers to (never NULL, or `NOT IN` would match nothing).
fn used_by<T: IntoIden + 'static, C: IntoIden + Copy + 'static>(
    table: T,
    column: C,
) -> SelectStatement {
    Query::select()
        .column(column)
        .from(table)
        .and_where(Expr::col(column).is_not_null())
        .to_owned()
}

/// Deletes the demo rows of `E` that no row of `used` refers to, then
/// clears the flag of the others: real data uses them, so they are real
/// from then on. (deleted, kept)
async fn prune<E: EntityTrait>(
    txn: &DatabaseTransaction,
    id: E::Column,
    flag: E::Column,
    used: Vec<SelectStatement>,
    step: &'static str,
) -> Result<(u64, u64)> {
    let mut unused = E::delete_many().filter(flag.eq(true));
    for refs in used {
        unused = unused.filter(id.not_in_subquery(refs));
    }
    let deleted = unused
        .exec(txn)
        .await
        .map_err(db_error(step))?
        .rows_affected;
    let kept = E::update_many()
        .col_expr(flag, Expr::value(false))
        .filter(flag.eq(true))
        .exec(txn)
        .await
        .map_err(db_error(step))?
        .rows_affected;
    Ok((deleted, kept))
}

/// Rows of `E` flagged `is_demo` (`flag`).
async fn flagged<E: EntityTrait>(
    db: &DatabaseConnection,
    flag: E::Column,
    step: &'static str,
) -> Result<u64>
where
    E::Model: Sync,
{
    E::find()
        .filter(flag.eq(true))
        .count(db)
        .await
        .map_err(db_error(step))
}

/// Inserts the demo accounts the businesses and partners name (role
/// provider and partner), every one flagged `is_demo`, all with
/// `password_hash`; an account whose username or email is taken already is
/// left out (its business or partner then has none). Their ids by
/// username, and how many were inserted.
async fn insert_accounts(
    txn: &DatabaseTransaction,
    data: &DemoData,
    password_hash: &str,
) -> Result<(HashMap<String, i32>, u64)> {
    let roles: HashMap<String, i32> = role::Entity::find()
        .select_only()
        .column(role::Column::Code)
        .column(role::Column::Id)
        .into_tuple::<(String, i32)>()
        .all(txn)
        .await
        .map_err(db_error("roles read"))?
        .into_iter()
        .collect();
    let now = Utc::now().fixed_offset();
    let accounts = data
        .businesses
        .iter()
        .filter_map(|b| Some((b.account.as_ref()?, ROLE_PROVIDER)))
        .chain(
            data.activity
                .partners
                .iter()
                .filter_map(|p| Some((p.account.as_ref()?, ROLE_PARTNER))),
        );
    let rows: Vec<user::ActiveModel> = accounts
        .filter_map(|(a, role)| {
            Some(user::ActiveModel {
                username: Set(a.username.to_lowercase()),
                email: Set(a.email.to_lowercase()),
                password_hash: Set(password_hash.to_owned()),
                full_name: Set(Some(a.full_name.clone())),
                role_id: Set(*roles.get(role)?),
                is_active: Set(true),
                email_verified_at: Set(a.verified.then_some(now)),
                is_demo: Set(true),
                ..Default::default()
            })
        })
        .collect();
    let mut inserted = 0;
    let mut rows = rows.into_iter().peekable();
    while rows.peek().is_some() {
        let chunk: Vec<user::ActiveModel> = rows.by_ref().take(CHUNK).collect();
        inserted += user::Entity::insert_many(chunk)
            .on_conflict(OnConflict::new().do_nothing().to_owned())
            .exec_without_returning(txn)
            .await
            .map_err(db_error("accounts"))?;
    }
    let ids = user::Entity::find()
        .select_only()
        .column(user::Column::Username)
        .column(user::Column::Id)
        .filter(user::Column::IsDemo.eq(true))
        .into_tuple::<(String, i32)>()
        .all(txn)
        .await
        .map_err(db_error("accounts read"))?
        .into_iter()
        .collect();
    Ok((ids, inserted))
}

/// Inserts the demo businesses, every one flagged `is_demo`, managed by
/// their demo account when they name one; a chunk per statement.
async fn insert_businesses(
    txn: &DatabaseTransaction,
    rows: &[DemoBusiness],
    accounts: &HashMap<String, i32>,
) -> Result<u64> {
    let mut inserted = 0;
    for chunk in rows.chunks(CHUNK) {
        let mut insert = Query::insert()
            .into_table(business::Entity)
            .columns([
                business::Column::Name.into_iden(),
                business::Column::Phone.into_iden(),
                business::Column::Description.into_iden(),
                business::Column::Address.into_iden(),
                Geo::Location.into_iden(),
                business::Column::WardCode.into_iden(),
                business::Column::IsVerified.into_iden(),
                business::Column::IsActive.into_iden(),
                business::Column::IsDemo.into_iden(),
                business::Column::OwnerId.into_iden(),
            ])
            .to_owned();
        for b in chunk {
            // The ward with the position, as a profile edit writes it.
            let (location, ward) = match (b.lat, b.lng) {
                (Some(lat), Some(lng)) => {
                    let point = || geography_from_text(&point_wkt(lat, lng));
                    (point(), ward_code_at(point(), ADMIN_UNIT_SNAP_M))
                }
                _ => (null(), null()),
            };
            let owner = b
                .account
                .as_ref()
                .and_then(|a| accounts.get(&a.username.to_lowercase()).copied());
            insert
                .values([
                    Expr::val(b.name.clone()).into(),
                    Expr::val(b.phone.clone()).into(),
                    Expr::val(b.description.clone()).into(),
                    Expr::val(b.address.clone()).into(),
                    location,
                    ward,
                    Expr::val(b.is_verified).into(),
                    Expr::val(true).into(),
                    Expr::val(true).into(),
                    Expr::val(owner).into(),
                ])
                .map_err(|e| Error::internal(format!("demo data businesses: {e}")))?;
        }
        inserted += txn
            .execute(txn.get_database_backend().build(&insert))
            .await
            .map(|r| r.rows_affected())
            .map_err(db_error("businesses"))?;
    }
    Ok(inserted)
}

/// Ids by slug of one type table.
macro_rules! slug_ids {
    ($table:ident, $txn:expr) => {
        $table::Entity::find()
            .select_only()
            .column($table::Column::Slug)
            .column($table::Column::Id)
            .into_tuple::<(String, i32)>()
            .all($txn)
            .await
            .map_err(db_error("types read"))?
            .into_iter()
            .collect::<HashMap<String, i32>>()
    };
}

/// Demo business ids by name, service and supply type ids by slug: what
/// the services, products and activity refer to.
async fn references(
    txn: &DatabaseTransaction,
    accounts: HashMap<String, i32>,
) -> Result<(HashMap<String, Vec<i32>>, Refs)> {
    let mut businesses: HashMap<String, Vec<i32>> = HashMap::new();
    for (id, name) in business::Entity::find()
        .select_only()
        .column(business::Column::Id)
        .column(business::Column::Name)
        .filter(is_demo())
        .into_tuple::<(i32, String)>()
        .all(txn)
        .await
        .map_err(db_error("businesses read"))?
    {
        businesses.entry(name).or_default().push(id);
    }
    let refs = Refs {
        businesses: businesses
            .iter()
            .filter_map(|(name, ids)| ids.first().map(|id| (name.clone(), *id)))
            .collect(),
        service_types: slug_ids!(service_type, txn),
        supply_types: slug_ids!(supply_type, txn),
        accounts,
    };
    Ok((businesses, refs))
}

/// Each (business id, type id) pair a demo row resolves to: none when its
/// type slug is gone.
fn resolve<'a>(
    businesses: &'a HashMap<String, Vec<i32>>,
    types: &'a HashMap<String, i32>,
    business: &str,
    slug: &str,
) -> impl Iterator<Item = (i32, i32)> + 'a {
    let category = types.get(slug).copied();
    businesses
        .get(business)
        .into_iter()
        .flatten()
        .filter_map(move |id| category.map(|c| (*id, c)))
}

#[async_trait]
impl DemoDataRepositoryTrait for DemoDataRepository {
    async fn count(&self) -> Result<DemoCounts> {
        let db = self.db.as_ref();
        let (sectors, alert_kinds, commodities, service_types, supply_types, locations, crops) = tokio::try_join!(
            flagged::<sector::Entity>(db, sector::Column::IsDemo, "sector count"),
            flagged::<alert_kind::Entity>(db, alert_kind::Column::IsDemo, "alert kind count"),
            flagged::<commodity::Entity>(db, commodity::Column::IsDemo, "commodity count"),
            flagged::<service_type::Entity>(db, service_type::Column::IsDemo, "type count"),
            flagged::<supply_type::Entity>(db, supply_type::Column::IsDemo, "type count"),
            flagged::<location::Entity>(db, location::Column::IsDemo, "location count"),
            flagged::<crop::Entity>(db, crop::Column::IsDemo, "crop count"),
        )?;
        let (businesses, services, products, inquiries, reviews, calls) = tokio::try_join!(
            async {
                business::Entity::find()
                    .filter(is_demo())
                    .count(db)
                    .await
                    .map_err(db_error("count"))
            },
            demo_rows::<service::Entity>(db, service::Column::BusinessId, "service count"),
            demo_rows::<product::Entity>(db, product::Column::BusinessId, "product count"),
            demo_rows::<inquiry::Entity>(db, inquiry::Column::BusinessId, "inquiry count"),
            demo_rows::<review::Entity>(db, review::Column::BusinessId, "review count"),
            demo_rows::<call_click::Entity>(db, call_click::Column::BusinessId, "call count"),
        )?;
        let (alerts, accounts) = tokio::try_join!(
            flagged::<alert::Entity>(db, alert::Column::IsDemo, "alert count"),
            flagged::<user::Entity>(db, user::Column::IsDemo, "account count"),
        )?;
        let (searches, prices, jobs, partners, contracts) = tokio::try_join!(
            flagged::<search_event::Entity>(db, search_event::Column::IsDemo, "search count"),
            flagged::<crop_price::Entity>(db, crop_price::Column::IsDemo, "price count"),
            flagged::<job_post::Entity>(db, job_post::Column::IsDemo, "job count"),
            flagged::<partner::Entity>(db, partner::Column::IsDemo, "partner count"),
            async {
                partner_contract::Entity::find()
                    .filter(
                        partner_contract::Column::PartnerId.in_subquery(
                            Query::select()
                                .column(partner::Column::Id)
                                .from(partner::Entity)
                                .and_where(partner::Column::IsDemo.eq(true))
                                .to_owned(),
                        ),
                    )
                    .count(db)
                    .await
                    .map_err(db_error("contract count"))
            },
        )?;
        Ok(DemoCounts {
            sectors,
            alert_kinds,
            commodities,
            categories: service_types + supply_types,
            locations,
            crops,
            businesses,
            services,
            products,
            inquiries,
            reviews,
            calls,
            searches,
            prices,
            jobs,
            partners,
            contracts,
            alerts,
            accounts,
        })
    }

    async fn load(&self) -> Result<DemoReport> {
        self.load_data(demo_data()?, None).await
    }

    async fn load_volume(&self, volume: &DemoVolume, password_hash: &str) -> Result<DemoReport> {
        let vocab = vocabulary()?;
        let wards = self.places(&vocab).await?;
        let data = demo_volume::generate(demo_data()?, &vocab, &wards, volume);
        self.load_data(data, Some(password_hash)).await
    }

    async fn remove(&self) -> Result<DemoRemoval> {
        // The cascade deletes the services and products, so their photos are
        // read first, in the same transaction.
        let txn = self.db.begin().await.map_err(db_error("begin"))?;
        let mut image_files = demo_photos::<service::Entity>(
            &txn,
            service::Column::BusinessId,
            service::Column::ImageFileId,
            "service images",
        )
        .await?;
        image_files.extend(
            demo_photos::<product::Entity>(
                &txn,
                product::Column::BusinessId,
                product::Column::ImageFileId,
                "product images",
            )
            .await?,
        );
        let businesses = business::Entity::delete_many()
            .filter(is_demo())
            .exec(&txn)
            .await
            .map(|r| r.rows_affected)
            .map_err(db_error("remove"))?;

        // Then the rest of the demo data with a flag of its own: partners
        // (contracts and sponsored shops follow), prices, job posts,
        // alerts, searches. Real rows are never flagged.
        let mut activity_removed = 0;
        activity_removed += partner::Entity::delete_many()
            .filter(partner::Column::IsDemo.eq(true))
            .exec(&txn)
            .await
            .map_err(db_error("remove partners"))?
            .rows_affected;
        activity_removed += crop_price::Entity::delete_many()
            .filter(crop_price::Column::IsDemo.eq(true))
            .exec(&txn)
            .await
            .map_err(db_error("remove prices"))?
            .rows_affected;
        activity_removed += job_post::Entity::delete_many()
            .filter(job_post::Column::IsDemo.eq(true))
            .exec(&txn)
            .await
            .map_err(db_error("remove jobs"))?
            .rows_affected;
        activity_removed += alert::Entity::delete_many()
            .filter(alert::Column::IsDemo.eq(true))
            .exec(&txn)
            .await
            .map_err(db_error("remove alerts"))?
            .rows_affected;
        activity_removed += search_event::Entity::delete_many()
            .filter(search_event::Column::IsDemo.eq(true))
            .exec(&txn)
            .await
            .map_err(db_error("remove searches"))?
            .rows_affected;
        // The demo accounts: their businesses and partners are gone, and
        // whatever else names an account (audit rows, files) keeps the row
        // with no one (SET NULL); their sessions go by cascade.
        activity_removed += user::Entity::delete_many()
            .filter(user::Column::IsDemo.eq(true))
            .exec(&txn)
            .await
            .map_err(db_error("remove accounts"))?
            .rows_affected;
        // Demo searches kept long enough to be counted into weekly totals.
        // A demo business's tap weeks go with the business (cascade).
        activity_removed += search_event_week::Entity::delete_many()
            .filter(search_event_week::Column::IsDemo.eq(true))
            .exec(&txn)
            .await
            .map_err(db_error("remove search weeks"))?
            .rows_affected;

        // The demo listings are gone, so whatever still refers to a demo
        // commodity, type, area or crop is real: those stay. Search events
        // are not counted as a use (their reference goes NULL, the search
        // stays). Crops first, then commodities: a commodity a kept crop or
        // supply uses stays too.
        let crops = prune::<crop::Entity>(
            &txn,
            crop::Column::Id,
            crop::Column::IsDemo,
            vec![used_by(crop_price::Entity, crop_price::Column::CropId)],
            "remove crops",
        )
        .await?;
        let service_types = prune::<service_type::Entity>(
            &txn,
            service_type::Column::Id,
            service_type::Column::IsDemo,
            vec![
                used_by(service::Entity, service::Column::ServiceTypeId),
                used_by(job_post::Entity, job_post::Column::ServiceTypeId),
                used_by(
                    partner_contract_service_type::Entity,
                    partner_contract_service_type::Column::ServiceTypeId,
                ),
            ],
            "remove service types",
        )
        .await?;
        let supply_types = prune::<supply_type::Entity>(
            &txn,
            supply_type::Column::Id,
            supply_type::Column::IsDemo,
            vec![
                used_by(product::Entity, product::Column::SupplyTypeId),
                used_by(
                    partner_contract_supply_type::Entity,
                    partner_contract_supply_type::Column::SupplyTypeId,
                ),
            ],
            "remove supply types",
        )
        .await?;
        let commodities = prune::<commodity::Entity>(
            &txn,
            commodity::Column::Id,
            commodity::Column::IsDemo,
            vec![
                used_by(crop::Entity, crop::Column::CommodityId),
                used_by(product::Entity, product::Column::CommodityId),
            ],
            "remove commodities",
        )
        .await?;
        let locations = prune::<location::Entity>(
            &txn,
            location::Column::Id,
            location::Column::IsDemo,
            vec![
                used_by(crop_price::Entity, crop_price::Column::LocationId),
                used_by(job_post::Entity, job_post::Column::LocationId),
            ],
            "remove locations",
        )
        .await?;
        // Last the sectors no commodity, type or alert left names.
        let sectors = prune::<sector::Entity>(
            &txn,
            sector::Column::Code,
            sector::Column::IsDemo,
            vec![
                used_by(commodity::Entity, commodity::Column::Sector),
                used_by(service_type::Entity, service_type::Column::Sector),
                used_by(supply_type::Entity, supply_type::Column::Sector),
                used_by(alert::Entity, alert::Column::Sector),
            ],
            "remove sectors",
        )
        .await?;
        let alert_kinds = prune::<alert_kind::Entity>(
            &txn,
            alert_kind::Column::Code,
            alert_kind::Column::IsDemo,
            vec![used_by(alert::Entity, alert::Column::Kind)],
            "remove alert kinds",
        )
        .await?;
        txn.commit().await.map_err(db_error("commit"))?;
        let parts = [
            crops,
            service_types,
            supply_types,
            commodities,
            locations,
            sectors,
            alert_kinds,
        ];
        Ok(DemoRemoval {
            businesses,
            activity_removed,
            catalogue_removed: parts.iter().map(|p| p.0).sum(),
            catalogue_kept: parts.iter().map(|p| p.1).sum(),
            image_files,
        })
    }
}

#[cfg(test)]
mod tests {
    //! The demo data must hold to the rules the forms enforce, and every
    //! name in it must refer to something it loads: a broken reference
    //! would only drop a row silently at load time.

    use std::collections::{HashMap, HashSet};

    use application::{
        constants::{ALERT_BODY_MAX, ALERT_TITLE_MAX, DEMAND_WEEKS, PRICE_PER_UNIT_MAX},
        validation::is_valid_vn_phone,
    };
    use domain::constants::{
        CALL_CHANNELS, CATEGORY_TYPE_PRODUCT, CATEGORY_TYPE_SERVICE, INQUIRY_STATUSES,
        INQUIRY_STATUSES_REVIEWABLE, JOB_STATUSES, PARTNER_KINDS,
    };

    use super::*;

    /// The codes of the demo sectors: what every `sector` in the file names.
    fn sector_codes(data: &DemoData) -> HashSet<&str> {
        data.sectors.iter().map(|s| s.code.as_str()).collect()
    }
    use crate::demo_activity::DemoArea;

    /// The file's demo data, and a generated sample of the demo data at
    /// volume: both must keep every rule below. `true`: generated.
    fn datasets() -> Result<Vec<(bool, DemoData)>> {
        Ok(vec![
            (false, demo_data()?),
            (true, crate::demo_volume::tests::sample(300, 42)?),
        ])
    }

    #[test]
    fn every_icon_is_one_the_forms_offer() -> Result<()> {
        for (_, data) in datasets()? {
            let icons = data
                .sectors
                .iter()
                .chain(&data.alert_kinds)
                .map(|c| &c.icon)
                .chain(data.commodities.iter().map(|c| &c.icon))
                .chain(
                    data.service_types
                        .iter()
                        .chain(&data.supply_types)
                        .map(|t| &t.icon),
                );
            for icon in icons {
                assert_eq!(
                    application::validation::icon(Some(icon))?.as_deref(),
                    Some(icon.as_str()),
                    "{icon} is not stored as the icon picker names it"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn listings_refer_to_demo_businesses_types_and_commodities() -> Result<()> {
        for (_, data) in datasets()? {
            let names: HashSet<&str> = data.businesses.iter().map(|b| b.name.as_str()).collect();
            let services: HashSet<&str> =
                data.service_types.iter().map(|c| c.slug.as_str()).collect();
            let supplies: HashSet<&str> =
                data.supply_types.iter().map(|c| c.slug.as_str()).collect();
            let commodities: HashSet<&str> =
                data.commodities.iter().map(|c| c.name.as_str()).collect();
            for s in &data.services {
                assert!(names.contains(s.business.as_str()), "{}", s.business);
                assert!(
                    services.contains(s.category_slug.as_str()),
                    "{} is not a demo service type",
                    s.category_slug
                );
            }
            for p in &data.products {
                assert!(names.contains(p.business.as_str()), "{}", p.business);
                assert!(
                    supplies.contains(p.category_slug.as_str()),
                    "{} is not a demo supply type",
                    p.category_slug
                );
                if let Some(c) = &p.commodity {
                    assert!(
                        commodities.contains(c.as_str()),
                        "{c} is not a demo commodity"
                    );
                }
            }
            for b in &data.businesses {
                assert!(is_valid_vn_phone(&b.phone), "{}: {}", b.name, b.phone);
            }
        }
        Ok(())
    }

    #[test]
    fn the_demo_catalogue_uses_known_sectors_and_unique_keys() -> Result<()> {
        for (_, data) in datasets()? {
            // A sector code is a slug, as the admin's are made (`slugify`).
            let mut sector_names = HashSet::new();
            for s in &data.sectors {
                assert_eq!(application::slug::slugify(&s.code), s.code);
                assert!(sector_names.insert(&s.name), "duplicate sector {}", s.name);
            }
            let sectors = sector_codes(&data);
            assert_eq!(sectors.len(), data.sectors.len(), "duplicate sector code");
            let mut commodities = HashSet::new();
            for c in &data.commodities {
                assert!(
                    sectors.contains(c.sector.as_str()),
                    "{}: {}",
                    c.name,
                    c.sector
                );
                assert!(commodities.insert(c.name.as_str()), "duplicate {}", c.name);
            }
            // Slugs are unique across both type tables (`taken_slugs`).
            let mut slugs = HashSet::new();
            for c in data.service_types.iter().chain(&data.supply_types) {
                assert!(slugs.insert(&c.slug), "duplicate slug {}", c.slug);
                if let Some(sector) = &c.sector {
                    assert!(sectors.contains(sector.as_str()), "{}: {sector}", c.slug);
                }
            }
            let mut labels = HashSet::new();
            for l in &data.locations {
                assert!(labels.insert(&l.label), "duplicate area {}", l.label);
            }
            let mut names = HashSet::new();
            for c in &data.crops {
                assert!(
                    commodities.contains(c.commodity.as_str()),
                    "{}: {}",
                    c.name,
                    c.commodity
                );
                assert!(names.insert(&c.name), "duplicate crop {}", c.name);
            }
        }
        Ok(())
    }

    #[test]
    fn leads_and_reviews_refer_to_their_shop_and_follow_the_review_rule() -> Result<()> {
        for (_, data) in datasets()? {
            let a = &data.activity;
            let shops: HashSet<&str> = data.businesses.iter().map(|b| b.name.as_str()).collect();
            let services: HashSet<(&str, &str)> = data
                .services
                .iter()
                .map(|s| (s.business.as_str(), s.name.as_str()))
                .collect();
            let products: HashSet<(&str, &str)> = data
                .products
                .iter()
                .map(|p| (p.business.as_str(), p.name.as_str()))
                .collect();
            let about = |shop: &str, service: &Option<String>, product: &Option<String>| {
                assert!(shops.contains(shop), "{shop} is not a demo business");
                if let Some(s) = service {
                    assert!(
                        services.contains(&(shop, s.as_str())),
                        "{shop} has no service {s}"
                    );
                }
                if let Some(p) = product {
                    assert!(
                        products.contains(&(shop, p.as_str())),
                        "{shop} has no product {p}"
                    );
                }
                assert!(
                    service.is_none() || product.is_none(),
                    "{shop}: one item at most"
                );
            };
            for c in &a.calls {
                about(&c.business, &c.service, &c.product);
                assert!(CALL_CHANNELS.contains(&c.channel.as_str()), "{}", c.channel);
                assert!(
                    c.weekly.len() <= DEMAND_WEEKS,
                    "{}: too many weeks",
                    c.business
                );
            }
            let mut answered: HashMap<(&str, &str), bool> = HashMap::new();
            for q in &a.inquiries {
                about(&q.business, &q.service, &q.product);
                assert!(
                    INQUIRY_STATUSES.contains(&q.status.as_str()),
                    "{}",
                    q.status
                );
                assert!(is_valid_vn_phone(&q.phone), "{}", q.phone);
                assert!(q.days_ago >= 0);
                let ok = INQUIRY_STATUSES_REVIEWABLE.contains(&q.status.as_str());
                *answered.entry((&q.business, &q.phone)).or_default() |= ok;
            }
            let mut reviewers = HashSet::new();
            for r in &a.reviews {
                assert!(
                    answered.get(&(r.business.as_str(), r.phone.as_str())) == Some(&true),
                    "{} reviews {} without an answered request",
                    r.phone,
                    r.business
                );
                assert!(
                    reviewers.insert((&r.business, &r.phone)),
                    "one review per phone and shop"
                );
                assert!((1..=5).contains(&r.rating));
                assert!(r.days_ago >= 1, "a reply comes a day after: {}", r.name);
            }
        }
        Ok(())
    }

    #[test]
    fn prices_jobs_searches_and_partners_refer_to_the_demo_catalogue() -> Result<()> {
        for (generated, data) in datasets()? {
            let a = &data.activity;
            let shops: HashSet<&str> = data.businesses.iter().map(|b| b.name.as_str()).collect();
            let kinds: HashMap<&str, &str> = data
                .service_types
                .iter()
                .map(|c| (c.slug.as_str(), CATEGORY_TYPE_SERVICE))
                .chain(
                    data.supply_types
                        .iter()
                        .map(|c| (c.slug.as_str(), CATEGORY_TYPE_PRODUCT)),
                )
                .collect();
            let areas: HashSet<&str> = data.locations.iter().map(|l| l.label.as_str()).collect();
            let crops: HashSet<&str> = data.crops.iter().map(|c| c.name.as_str()).collect();
            let mut pairs = HashSet::new();
            for p in &a.prices {
                assert!(
                    pairs.insert((p.crop.as_str(), p.location.as_str())),
                    "one entry per crop and area: {} {}",
                    p.crop,
                    p.location
                );
                assert!(crops.contains(p.crop.as_str()), "{}", p.crop);
                assert!(areas.contains(p.location.as_str()), "{}", p.location);
                let mut days = HashSet::new();
                for (days_ago, low, high) in &p.points {
                    assert!(*days_ago >= 0, "no price after today");
                    assert!(days.insert(days_ago), "one price per crop, area and day");
                    assert!(0 < *low && low <= high && *high <= PRICE_PER_UNIT_MAX);
                }
            }
            for j in &a.jobs {
                assert_eq!(
                    kinds.get(j.category_slug.as_str()),
                    Some(&CATEGORY_TYPE_SERVICE),
                    "{}",
                    j.category_slug
                );
                assert!(areas.contains(j.location.as_str()), "{}", j.location);
                assert!(JOB_STATUSES.contains(&j.status.as_str()), "{}", j.status);
                assert!(is_valid_vn_phone(&j.phone), "{}", j.phone);
            }
            for s in &a.searches {
                assert!(areas.contains(s.location.as_str()), "{}", s.location);
                if let Some(slug) = &s.category_slug {
                    assert!(kinds.contains_key(slug.as_str()), "{slug}");
                }
                assert!(s.weekly.len() <= DEMAND_WEEKS);
            }
            for p in &a.partners {
                assert!(PARTNER_KINDS.contains(&p.kind.as_str()), "{}", p.kind);
                assert!(is_valid_vn_phone(&p.phone), "{}", p.phone);
                for c in &p.contracts {
                    let (DemoArea::WardOf(shop) | DemoArea::ProvinceOf(shop)) = &c.area;
                    assert!(shops.contains(shop.as_str()), "{shop}");
                    assert!(
                        c.ends_in_days >= c.starts_in_days,
                        "{}: ends before it starts",
                        p.name
                    );
                    assert!(c.monthly_fee >= 0);
                    for slug in &c.categories {
                        assert!(kinds.contains_key(slug.as_str()), "{slug}");
                    }
                    for s in &c.sponsored {
                        assert!(shops.contains(s.as_str()), "{s}");
                    }
                }
            }
            for al in &a.alerts {
                assert!(
                    data.alert_kinds.iter().any(|k| k.code == al.kind),
                    "{} is not a demo alert kind",
                    al.kind
                );
                if let Some(sector) = &al.sector {
                    assert!(sector_codes(&data).contains(sector.as_str()), "{sector}");
                }
                if let Some(shop) = &al.province_of {
                    assert!(shops.contains(shop.as_str()), "{shop}");
                }
                assert!(al.ends_in_days >= al.starts_in_days, "{}", al.title);
                // The file's alerts are all in force; generated ones mostly past.
                assert!(
                    generated || (al.starts_in_days <= 0 && al.ends_in_days >= 0),
                    "in force today"
                );
                assert!(
                    al.title.chars().count() <= ALERT_TITLE_MAX
                        && al.body.chars().count() <= ALERT_BODY_MAX
                );
            }
        }
        Ok(())
    }
}
