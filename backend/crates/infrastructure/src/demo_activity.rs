//! The activity part of the demo data: what happened around the demo
//! listings, so the reports, the price board, the jobs page and the
//! partners' pages can be tried on figures that agree with each other.
//!
//! Every row here either belongs to a demo business (inquiries, reviews,
//! call taps: removed by cascade) or to a demo partner (contracts, their
//! categories, sponsored shops: likewise), or carries its own `is_demo`
//! (partners, prices, job posts, searches). Dates are relative to the day
//! of loading: leads and searches by report week (`report_weeks`, the
//! demand report's own), the rest in days from today, so the data stays
//! fresh whenever it is loaded. Areas are not typed codes: a contract's
//! area is the ward or province a demo shop lies in, a search's ward is
//! that of an area preset, both looked up as the report does
//! (`ward_code_at`), so a sponsored shop is always inside its contract's
//! area. A row whose reference cannot be found is left out, and the
//! returned counts show it.

use std::collections::HashMap;

use application::{
    constants::{ADMIN_UNIT_SNAP_M, VN_UTC_OFFSET_SECS},
    features::Stage,
    hashing::sha256_hex,
    posts::STATUS_PUBLISHED,
    usecases::demand_usecase::{report_weeks, vn_today},
    validation::point_wkt,
};
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use domain::{
    constants::INQUIRY_STATUSES_REVIEWABLE, models::catalogue::DemoCounts, Error, Result,
};
use sea_orm::{
    sea_query::{Expr, IntoIden, Query},
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, DbErr, EntityTrait,
    FromQueryResult, IntoActiveModel, QueryFilter, QuerySelect, Set,
};
use serde::Deserialize;

use crate::{
    demo_data_repository::demo_business_ids,
    entities::{
        alert, business, call_click, commodity, crop, crop_price, info_page, info_page_revision,
        inquiry, job_post, location, partner, partner_contract, partner_contract_service_type,
        partner_contract_supply_type, partner_sponsored, post, product, review, search_event,
        service, ward,
    },
    repositories::admin_unit_repository::ward_code_at,
    sql::{geography_from_text, Geo, QueryAs},
};

#[derive(Deserialize)]
pub(crate) struct Activity {
    pub(crate) calls: Vec<DemoCalls>,
    pub(crate) inquiries: Vec<DemoInquiry>,
    pub(crate) reviews: Vec<DemoReview>,
    pub(crate) searches: Vec<DemoSearches>,
    pub(crate) prices: Vec<DemoPrices>,
    pub(crate) jobs: Vec<DemoJob>,
    pub(crate) partners: Vec<DemoPartner>,
    pub(crate) alerts: Vec<DemoAlert>,
    /// Articles, published; the volume generator keeps these as they are.
    #[serde(default)]
    pub(crate) posts: Vec<DemoPost>,
    /// Information pages to start from (terms, policies, contact), in
    /// preview: admins see them, farmers do not, until one is turned on.
    #[serde(default)]
    pub(crate) info_pages: Vec<DemoInfoPage>,
}

/// Call or Zalo taps on a shop (or one of its services or products), by
/// report week, oldest first: the last `weekly.len()` report weeks.
#[derive(Deserialize)]
pub(crate) struct DemoCalls {
    pub(crate) business: String,
    pub(crate) service: Option<String>,
    pub(crate) product: Option<String>,
    pub(crate) channel: String,
    pub(crate) weekly: Vec<u32>,
}

#[derive(Deserialize)]
pub(crate) struct DemoInquiry {
    pub(crate) business: String,
    pub(crate) service: Option<String>,
    pub(crate) product: Option<String>,
    pub(crate) topic: String,
    pub(crate) name: String,
    pub(crate) phone: String,
    pub(crate) note: String,
    pub(crate) status: String,
    pub(crate) days_ago: i64,
}

/// A review from the phone of a request of that shop it answered.
#[derive(Deserialize)]
pub(crate) struct DemoReview {
    pub(crate) business: String,
    pub(crate) phone: String,
    pub(crate) name: String,
    pub(crate) rating: i16,
    pub(crate) comment: String,
    pub(crate) reply: Option<String>,
    pub(crate) visible: bool,
    pub(crate) days_ago: i64,
}

/// The same search near an area preset, by report week like the taps.
#[derive(Deserialize)]
pub(crate) struct DemoSearches {
    pub(crate) query: String,
    pub(crate) category_slug: Option<String>,
    pub(crate) location: String,
    pub(crate) results: i32,
    pub(crate) weekly: Vec<u32>,
}

/// A crop's prices in one area: `[days ago, low, high]`.
#[derive(Deserialize)]
pub(crate) struct DemoPrices {
    pub(crate) crop: String,
    pub(crate) location: String,
    pub(crate) points: Vec<(i64, i64, i64)>,
}

#[derive(Deserialize)]
pub(crate) struct DemoJob {
    pub(crate) category_slug: String,
    pub(crate) location: String,
    pub(crate) days_ago: i64,
    pub(crate) description: String,
    pub(crate) name: String,
    pub(crate) phone: String,
    pub(crate) status: String,
}

/// An alert in force from `starts_in_days` to `ends_in_days` (from today),
/// over the province a demo shop lies in, or the whole region.
#[derive(Deserialize)]
pub(crate) struct DemoAlert {
    pub(crate) kind: String,
    pub(crate) sector: Option<String>,
    pub(crate) province_of: Option<String>,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) starts_in_days: i64,
    pub(crate) ends_in_days: i64,
}

/// An information page and its text, in force since yesterday.
#[derive(Deserialize)]
pub(crate) struct DemoInfoPage {
    pub(crate) code: String,
    pub(crate) title: String,
    pub(crate) icon: Option<String>,
    pub(crate) footer_group: String,
    pub(crate) in_footer: bool,
    pub(crate) body: String,
}

/// An article published `days_ago`, for one sector, the province a demo
/// shop lies in and one commodity (by name), each optional.
#[derive(Deserialize)]
pub(crate) struct DemoPost {
    pub(crate) kind: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) body: String,
    pub(crate) sector: Option<String>,
    pub(crate) province_of: Option<String>,
    pub(crate) commodity: Option<String>,
    #[serde(default)]
    pub(crate) pinned: bool,
    pub(crate) days_ago: i64,
}

#[derive(Deserialize)]
pub(crate) struct DemoPartner {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) contact_name: String,
    pub(crate) phone: String,
    pub(crate) email: String,
    pub(crate) contracts: Vec<DemoContract>,
    /// Its report account (role partner); loaded only at volume.
    #[serde(default)]
    pub(crate) account: Option<DemoAccount>,
}

/// A demo account (flagged `is_demo`): a provider's managing account or a
/// partner's report account. Loaded only at volume, where every demo
/// account signs in with the one password given.
#[derive(Deserialize, Clone)]
pub(crate) struct DemoAccount {
    pub(crate) email: String,
    pub(crate) full_name: String,
    /// Whether its email address is confirmed.
    pub(crate) verified: bool,
}

/// The area of a contract: the ward, or the whole province, a demo shop
/// lies in.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DemoArea {
    WardOf(String),
    ProvinceOf(String),
}

#[derive(Deserialize)]
pub(crate) struct DemoContract {
    pub(crate) area: DemoArea,
    pub(crate) categories: Vec<String>,
    /// Days from today (negative: in the past), both days included.
    pub(crate) starts_in_days: i64,
    pub(crate) ends_in_days: i64,
    pub(crate) monthly_fee: i64,
    pub(crate) note: String,
    pub(crate) sponsored: Vec<String>,
}

/// What the activity refers to, resolved once the catalogue and listings
/// are in: ids by name, slug or label.
pub(crate) struct Refs {
    /// Demo businesses by name.
    pub(crate) businesses: HashMap<String, i32>,
    /// Service types by slug.
    pub(crate) service_types: HashMap<String, i32>,
    /// Supply types by slug.
    pub(crate) supply_types: HashMap<String, i32>,
    /// Demo accounts by email.
    pub(crate) accounts: HashMap<String, i32>,
}

#[derive(FromQueryResult)]
struct CodeRow {
    ward_code: Option<String>,
}

/// Where a business or a preset lies: its ward and that ward's province.
#[derive(Clone)]
struct Place {
    ward: String,
    province: String,
}

fn db_error(step: &'static str) -> impl Fn(DbErr) -> Error {
    move |e| Error::internal(format!("demo activity {step}: {e}"))
}

/// Hour `hour` (Vietnam) of `day`, as an instant.
fn vn_instant(day: NaiveDate, hour: u32) -> DateTime<Utc> {
    let local = day.and_hms_opt(hour.min(23), 0, 0).unwrap_or_default();
    Utc.from_utc_datetime(&local) - Duration::seconds(i64::from(VN_UTC_OFFSET_SECS))
}

/// The instants of `weekly` events, aligned to the last report weeks: the
/// k-th of a week on its (k mod 7)-th day, spread over working hours.
fn weekly_instants(weekly: &[u32], weeks: &[NaiveDate]) -> Vec<DateTime<Utc>> {
    let skip = weeks.len().saturating_sub(weekly.len());
    weeks
        .iter()
        .skip(skip)
        .zip(weekly)
        .flat_map(|(monday, n)| {
            (0..*n).map(move |k| {
                vn_instant(*monday + Duration::days(i64::from(k % 7)), 7 + (k * 5) % 12)
            })
        })
        .collect()
}

/// The ward of a point and its province, as the demand report places it.
async fn place_of(txn: &DatabaseTransaction, lat: f64, lng: f64) -> Result<Option<Place>> {
    let point = geography_from_text(&point_wkt(lat, lng));
    let query = Query::select()
        .expr_as(ward_code_at(point, ADMIN_UNIT_SNAP_M), QueryAs::WardCode)
        .to_owned();
    let code = CodeRow::find_by_statement(txn.get_database_backend().build(&query))
        .one(txn)
        .await
        .map_err(db_error("ward"))?
        .and_then(|r| r.ward_code);
    let Some(code) = code else {
        return Ok(None);
    };
    Ok(ward::Entity::find_by_id(code.clone())
        .one(txn)
        .await
        .map_err(db_error("ward"))?
        .map(|w| Place {
            ward: code,
            province: w.province_code,
        }))
}

/// Rows written per statement: far under Postgres' 65 535 parameters for
/// the widest demo row, and few statements for the demo data at volume.
pub(crate) const CHUNK: usize = 1_000;

/// Inserts `rows` a chunk at a time; the number inserted.
pub(crate) async fn insert_chunks<A>(
    txn: &DatabaseTransaction,
    rows: Vec<A>,
    step: &'static str,
) -> Result<u64>
where
    A: ActiveModelTrait + Send,
    <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
{
    let total = rows.len() as u64;
    let mut rows = rows.into_iter().peekable();
    while rows.peek().is_some() {
        let chunk: Vec<A> = rows.by_ref().take(CHUNK).collect();
        <A::Entity as EntityTrait>::insert_many(chunk)
            .exec_without_returning(txn)
            .await
            .map_err(db_error(step))?;
    }
    Ok(total)
}

/// Where each demo business lies, by name: the ward written with its
/// position (`ward_code`, as the report places it) and that ward's
/// province. One read, however many businesses.
async fn shop_places(txn: &DatabaseTransaction) -> Result<HashMap<String, Place>> {
    let provinces: HashMap<String, String> = ward::Entity::find()
        .select_only()
        .column(ward::Column::Code)
        .column(ward::Column::ProvinceCode)
        .into_tuple::<(String, String)>()
        .all(txn)
        .await
        .map_err(db_error("wards"))?
        .into_iter()
        .collect();
    Ok(business::Entity::find()
        .select_only()
        .column(business::Column::Name)
        .column(business::Column::WardCode)
        .filter(business::Column::IsDemo.eq(true))
        .filter(business::Column::WardCode.is_not_null())
        .into_tuple::<(String, String)>()
        .all(txn)
        .await
        .map_err(db_error("business wards"))?
        .into_iter()
        .filter_map(|(name, ward)| {
            let province = provinces.get(&ward)?.clone();
            Some((name, Place { ward, province }))
        })
        .collect())
}

/// Loads the activity; returns its counts (the listing fields left at 0).
pub(crate) async fn insert(
    txn: &DatabaseTransaction,
    data: &Activity,
    refs: &Refs,
) -> Result<DemoCounts> {
    let now = Utc::now();
    let today = vn_today(now)?;
    let weeks: Vec<NaiveDate> = report_weeks(now)?.iter().map(|w| w.starts_on).collect();
    let ago = |days: i64| (now - Duration::days(days)).fixed_offset();

    // Ids of the demo services and products by (business, name).
    let services: HashMap<(i32, String), i32> = service::Entity::find()
        .filter(service::Column::BusinessId.in_subquery(demo_business_ids()))
        .all(txn)
        .await
        .map_err(db_error("services"))?
        .into_iter()
        .map(|s| ((s.business_id, s.name), s.id))
        .collect();
    let products: HashMap<(i32, String), i32> = product::Entity::find()
        .filter(product::Column::BusinessId.in_subquery(demo_business_ids()))
        .all(txn)
        .await
        .map_err(db_error("products"))?
        .into_iter()
        .map(|p| ((p.business_id, p.name), p.id))
        .collect();
    let presets: HashMap<String, location::Model> = location::Entity::find()
        .all(txn)
        .await
        .map_err(db_error("locations"))?
        .into_iter()
        .map(|l| (l.label.clone(), l))
        .collect();
    let crops: HashMap<String, i32> = crop::Entity::find()
        .all(txn)
        .await
        .map_err(db_error("crops"))?
        .into_iter()
        .map(|c| (c.name, c.id))
        .collect();
    let shop_places = shop_places(txn).await?;
    let item = |business: i32, service: &Option<String>, product: &Option<String>| {
        (
            service
                .as_ref()
                .and_then(|n| services.get(&(business, n.clone())).copied()),
            product
                .as_ref()
                .and_then(|n| products.get(&(business, n.clone())).copied()),
        )
    };
    let mut counts = DemoCounts::default();

    // Call and Zalo taps.
    let mut taps = Vec::new();
    for c in &data.calls {
        let Some(&business_id) = refs.businesses.get(&c.business) else {
            continue;
        };
        let (service_id, product_id) = item(business_id, &c.service, &c.product);
        for at in weekly_instants(&c.weekly, &weeks) {
            taps.push(call_click::ActiveModel {
                business_id: Set(business_id),
                service_id: Set(service_id),
                product_id: Set(product_id),
                channel: Set(c.channel.clone()),
                created_on: Set(at.fixed_offset()),
                ..Default::default()
            });
        }
    }
    counts.calls = insert_chunks(txn, taps, "calls").await?;

    // Contact requests, then the reviews their answered phones may write.
    let mut requests = Vec::new();
    for q in &data.inquiries {
        let Some(&business_id) = refs.businesses.get(&q.business) else {
            continue;
        };
        let (service_id, product_id) = item(business_id, &q.service, &q.product);
        requests.push(inquiry::ActiveModel {
            business_id: Set(business_id),
            service_id: Set(service_id),
            product_id: Set(product_id),
            topic: Set(q.topic.clone()),
            requester_name: Set(q.name.clone()),
            requester_phone: Set(q.phone.clone()),
            note: Set(Some(q.note.clone())),
            status: Set(q.status.clone()),
            created_on: Set(ago(q.days_ago)),
            updated_on: Set(ago(q.days_ago)),
            ..Default::default()
        });
    }
    counts.inquiries = insert_chunks(txn, requests, "inquiries").await?;
    // The answered requests, read back: a review links to one of them.
    let answered: HashMap<(i32, String), i32> = inquiry::Entity::find()
        .select_only()
        .column(inquiry::Column::BusinessId)
        .column(inquiry::Column::RequesterPhone)
        .column(inquiry::Column::Id)
        .filter(inquiry::Column::BusinessId.in_subquery(demo_business_ids()))
        .filter(inquiry::Column::Status.is_in(INQUIRY_STATUSES_REVIEWABLE.iter().copied()))
        .into_tuple::<(i32, String, i32)>()
        .all(txn)
        .await
        .map_err(db_error("answered inquiries"))?
        .into_iter()
        .map(|(business, phone, id)| ((business, phone), id))
        .collect();
    let mut reviews = Vec::new();
    for r in &data.reviews {
        let Some(&business_id) = refs.businesses.get(&r.business) else {
            continue;
        };
        // The rule the review form enforces: an answered request first.
        let Some(&inquiry_id) = answered.get(&(business_id, r.phone.clone())) else {
            continue;
        };
        reviews.push(review::ActiveModel {
            business_id: Set(business_id),
            inquiry_id: Set(Some(inquiry_id)),
            rating: Set(r.rating),
            comment: Set(Some(r.comment.clone())),
            reviewer_name: Set(r.name.clone()),
            reviewer_phone_hash: Set(sha256_hex(&r.phone)),
            is_visible: Set(r.visible),
            created_on: Set(ago(r.days_ago)),
            updated_on: Set(ago(r.days_ago)),
            reply: Set(r.reply.clone()),
            replied_at: Set(r.reply.as_ref().map(|_| ago(r.days_ago - 1))),
            ..Default::default()
        });
    }
    counts.reviews = insert_chunks(txn, reviews, "reviews").await?;

    // Searches near a preset, in its ward (looked up once per preset).
    let mut preset_wards: HashMap<&str, Option<String>> = HashMap::new();
    let mut searches = Vec::new();
    for s in &data.searches {
        let Some(preset) = presets.get(&s.location) else {
            continue;
        };
        let ward_code = match preset_wards.get(s.location.as_str()) {
            Some(code) => code.clone(),
            None => {
                let code = place_of(txn, preset.lat, preset.lng).await?.map(|p| p.ward);
                preset_wards.insert(&s.location, code.clone());
                code
            }
        };
        let slug = s.category_slug.as_deref().unwrap_or_default();
        let service_type_id = refs.service_types.get(slug).copied();
        let supply_type_id = refs.supply_types.get(slug).copied();
        for at in weekly_instants(&s.weekly, &weeks) {
            searches.push(search_event::ActiveModel {
                query: Set(Some(s.query.clone())),
                service_type_id: Set(service_type_id),
                supply_type_id: Set(supply_type_id),
                ward_code: Set(ward_code.clone()),
                results: Set(s.results),
                created_on: Set(at.fixed_offset()),
                is_demo: Set(true),
                ..Default::default()
            });
        }
    }
    counts.searches = insert_chunks(txn, searches, "searches").await?;

    // Farm-gate prices.
    let mut prices = Vec::new();
    for p in &data.prices {
        let (Some(&crop_id), Some(preset)) = (crops.get(&p.crop), presets.get(&p.location)) else {
            continue;
        };
        for (days_ago, low, high) in &p.points {
            prices.push(crop_price::ActiveModel {
                crop_id: Set(crop_id),
                location_id: Set(preset.id),
                observed_on: Set(today - Duration::days(*days_ago)),
                price_low: Set(*low),
                price_high: Set(*high),
                is_demo: Set(true),
                ..Default::default()
            });
        }
    }
    counts.prices = insert_chunks(txn, prices, "prices").await?;

    // Job posts, at their preset's point, a chunk per statement.
    for chunk in data.jobs.chunks(CHUNK) {
        let mut insert = Query::insert()
            .into_table(job_post::Entity)
            .columns([
                job_post::Column::ServiceTypeId.into_iden(),
                job_post::Column::LocationId.into_iden(),
                Geo::Location.into_iden(),
                job_post::Column::Description.into_iden(),
                job_post::Column::RequesterName.into_iden(),
                job_post::Column::RequesterPhone.into_iden(),
                job_post::Column::Status.into_iden(),
                job_post::Column::CreatedOn.into_iden(),
                job_post::Column::IsDemo.into_iden(),
            ])
            .to_owned();
        let mut rows = 0;
        for j in chunk {
            let (Some(&category_id), Some(preset)) = (
                refs.service_types.get(&j.category_slug),
                presets.get(&j.location),
            ) else {
                continue;
            };
            insert
                .values([
                    Expr::val(category_id).into(),
                    Expr::val(preset.id).into(),
                    geography_from_text(&point_wkt(preset.lat, preset.lng)),
                    Expr::val(j.description.clone()).into(),
                    Expr::val(j.name.clone()).into(),
                    Expr::val(j.phone.clone()).into(),
                    Expr::val(j.status.clone()).into(),
                    Expr::val(ago(j.days_ago)).into(),
                    Expr::val(true).into(),
                ])
                .map_err(|e| Error::internal(format!("demo activity jobs: {e}")))?;
            rows += 1;
        }
        if rows > 0 {
            txn.execute(txn.get_database_backend().build(&insert))
                .await
                .map_err(db_error("jobs"))?;
        }
        counts.jobs += rows;
    }

    // Partners, their contracts over a demo shop's ward or province, and
    // the shops of that area they sponsor.
    for p in &data.partners {
        let partner_id = partner::Entity::insert(partner::ActiveModel {
            name: Set(p.name.clone()),
            kind: Set(p.kind.clone()),
            contact_name: Set(Some(p.contact_name.clone())),
            phone: Set(Some(p.phone.clone())),
            email: Set(Some(p.email.clone())),
            account_id: Set(p
                .account
                .as_ref()
                .and_then(|a| refs.accounts.get(&a.email).copied())),
            is_demo: Set(true),
            ..Default::default()
        })
        .exec(txn)
        .await
        .map_err(db_error("partners"))?
        .last_insert_id;
        counts.partners += 1;
        for c in &p.contracts {
            let (shop, whole_province) = match &c.area {
                DemoArea::WardOf(shop) => (shop, false),
                DemoArea::ProvinceOf(shop) => (shop, true),
            };
            let Some(area) = shop_places.get(shop) else {
                continue;
            };
            let contract_id = partner_contract::Entity::insert(partner_contract::ActiveModel {
                partner_id: Set(partner_id),
                province_code: Set(area.province.clone()),
                ward_code: Set((!whole_province).then(|| area.ward.clone())),
                starts_on: Set(today + Duration::days(c.starts_in_days)),
                ends_on: Set(today + Duration::days(c.ends_in_days)),
                monthly_fee: Set(c.monthly_fee),
                note: Set(Some(c.note.clone())),
                ..Default::default()
            })
            .exec(txn)
            .await
            .map_err(db_error("contracts"))?
            .last_insert_id;
            counts.contracts += 1;
            let services: Vec<_> = c
                .categories
                .iter()
                .filter_map(|slug| refs.service_types.get(slug))
                .map(|&id| partner_contract_service_type::ActiveModel {
                    contract_id: Set(contract_id),
                    service_type_id: Set(id),
                })
                .collect();
            if !services.is_empty() {
                partner_contract_service_type::Entity::insert_many(services)
                    .exec_without_returning(txn)
                    .await
                    .map_err(db_error("contract service types"))?;
            }
            let supplies: Vec<_> = c
                .categories
                .iter()
                .filter_map(|slug| refs.supply_types.get(slug))
                .map(|&id| partner_contract_supply_type::ActiveModel {
                    contract_id: Set(contract_id),
                    supply_type_id: Set(id),
                })
                .collect();
            if !supplies.is_empty() {
                partner_contract_supply_type::Entity::insert_many(supplies)
                    .exec_without_returning(txn)
                    .await
                    .map_err(db_error("contract supply types"))?;
            }
            // Only shops inside the area, as the admin's form requires.
            let sponsored: Vec<_> = c
                .sponsored
                .iter()
                .filter(|name| {
                    shop_places.get(*name).is_some_and(|at| {
                        if whole_province {
                            at.province == area.province
                        } else {
                            at.ward == area.ward
                        }
                    })
                })
                .filter_map(|name| refs.businesses.get(name))
                .map(|&business_id| partner_sponsored::ActiveModel {
                    contract_id: Set(contract_id),
                    business_id: Set(business_id),
                })
                .collect();
            if !sponsored.is_empty() {
                partner_sponsored::Entity::insert_many(sponsored)
                    .exec_without_returning(txn)
                    .await
                    .map_err(db_error("sponsored"))?;
            }
        }
    }

    // Alerts, over a demo shop's province (the whole region when it has none).
    let alerts: Vec<_> = data
        .alerts
        .iter()
        .map(|a| alert::ActiveModel {
            kind: Set(a.kind.clone()),
            sector: Set(a.sector.clone()),
            province_code: Set(a
                .province_of
                .as_ref()
                .and_then(|shop| shop_places.get(shop))
                .map(|at| at.province.clone())),
            title: Set(a.title.clone()),
            body: Set(a.body.clone()),
            starts_on: Set(today + Duration::days(a.starts_in_days)),
            ends_on: Set(today + Duration::days(a.ends_in_days)),
            is_demo: Set(true),
            ..Default::default()
        })
        .collect();
    counts.alerts = insert_chunks(txn, alerts, "alerts").await?;

    // Articles, published some days ago; a commodity by its name.
    let commodities: HashMap<String, i32> = commodity::Entity::find()
        .all(txn)
        .await
        .map_err(db_error("commodities"))?
        .into_iter()
        .map(|c| (c.name, c.id))
        .collect();
    let posts: Vec<_> = data
        .posts
        .iter()
        .map(|p| post::ActiveModel {
            kind: Set(p.kind.clone()),
            title: Set(p.title.clone()),
            summary: Set(p.summary.clone()),
            body: Set(p.body.clone()),
            sector: Set(p.sector.clone()),
            province_code: Set(p
                .province_of
                .as_ref()
                .and_then(|shop| shop_places.get(shop))
                .map(|at| at.province.clone())),
            commodity_id: Set(p
                .commodity
                .as_ref()
                .and_then(|name| commodities.get(name).copied())),
            status: Set(STATUS_PUBLISHED.to_owned()),
            published_on: Set(Some(ago(p.days_ago))),
            is_pinned: Set(p.pinned),
            is_demo: Set(true),
            ..Default::default()
        })
        .collect();
    counts.posts = insert_chunks(txn, posts, "posts").await?;

    // Information pages, each with one version; a page whose address is
    // taken already (a real one) is passed over.
    for (order, page) in data.info_pages.iter().enumerate() {
        let taken = info_page::Entity::find()
            .filter(info_page::Column::Code.eq(page.code.clone()))
            .one(txn)
            .await
            .map_err(db_error("info pages"))?
            .is_some();
        if taken {
            continue;
        }
        let id = info_page::Entity::insert(info_page::ActiveModel {
            code: Set(page.code.clone()),
            title: Set(page.title.clone()),
            icon: Set(page.icon.clone()),
            stage: Set(Stage::Preview.code().to_owned()),
            in_footer: Set(page.in_footer),
            footer_group: Set(page.footer_group.clone()),
            display_order: Set(i32::try_from(order).unwrap_or(i32::MAX)),
            is_demo: Set(true),
            ..Default::default()
        })
        .exec(txn)
        .await
        .map_err(db_error("info page"))?
        .last_insert_id;
        info_page_revision::Entity::insert(info_page_revision::ActiveModel {
            page_id: Set(id),
            body: Set(page.body.clone()),
            effective_on: Set(today - Duration::days(1)),
            ..Default::default()
        })
        .exec(txn)
        .await
        .map_err(db_error("info page text"))?;
        counts.info_pages += 1;
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_events_fall_in_their_report_weeks() {
        let first = NaiveDate::from_ymd_opt(2026, 9, 7).unwrap_or_default();
        let mondays: Vec<NaiveDate> = (0..8).map(|w| first + Duration::weeks(w)).collect();
        let vn = |t: &DateTime<Utc>| {
            (*t + Duration::seconds(i64::from(VN_UTC_OFFSET_SECS))).date_naive()
        };
        let days: Vec<NaiveDate> = weekly_instants(&[1, 0, 9], &mondays)
            .iter()
            .map(vn)
            .collect();
        assert_eq!(days.len(), 10, "the last three weeks: 1 + 0 + 9");
        // The first lands in week 6, the other nine in week 8, Monday to Sunday.
        let week6 = first + Duration::weeks(5);
        let week8 = first + Duration::weeks(7);
        assert_eq!(days.first(), Some(&week6));
        for day in days.iter().skip(1) {
            assert!(*day >= week8 && *day < week8 + Duration::days(7), "{day}");
        }
    }
}
