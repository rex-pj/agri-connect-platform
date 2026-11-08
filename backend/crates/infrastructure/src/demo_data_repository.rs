//! Sample listings for testing, loaded only on request.
//!
//! The listings are data (`demo/data.json`), written through the query
//! builder. Services and products name their business and category slug; a
//! slug an admin renamed makes its demo rows silently drop out, and the
//! returned counts show it.

use std::{collections::HashMap, sync::Arc};

use application::validation::point_wkt;
use async_trait::async_trait;
use domain::{
    models::catalogue::{DemoRemoval, DemoReport},
    repositories::demo_data_repository_trait::DemoDataRepositoryTrait,
    Error, Result,
};
use sea_orm::{
    sea_query::{Expr, IntoIden, LockType, Query, SimpleExpr},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait,
    JoinType, PaginatorTrait, QueryFilter, QuerySelect, QueryTrait, RelationTrait, Set,
    TransactionTrait,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    entities::{business, category, product, service},
    sql::{geography_from_text, null, Geo},
};

const DEMO_DATA: &str = include_str!("demo/data.json");

#[derive(Deserialize)]
struct DemoData {
    businesses: Vec<DemoBusiness>,
    services: Vec<DemoService>,
    products: Vec<DemoProduct>,
}

#[derive(Deserialize)]
struct DemoBusiness {
    name: String,
    phone: String,
    description: String,
    address: String,
    lat: Option<f64>,
    lng: Option<f64>,
    is_verified: bool,
}

#[derive(Deserialize)]
struct DemoService {
    business: String,
    category_slug: String,
    name: String,
    description: String,
    price_note: String,
}

#[derive(Deserialize)]
struct DemoProduct {
    business: String,
    category_slug: String,
    name: String,
    description: String,
    price: i64,
    unit: String,
}

pub struct DemoDataRepository {
    db: Arc<DatabaseConnection>,
}

impl DemoDataRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

fn db_error(step: &'static str) -> impl Fn(DbErr) -> Error {
    move |e| Error::internal(format!("demo data {step}: {e}"))
}

/// `businesses.is_demo`: the rows this repository owns.
fn is_demo() -> SimpleExpr {
    business::Column::IsDemo.eq(true)
}

/// Inserts the demo businesses, every one flagged `is_demo`.
async fn insert_businesses(txn: &DatabaseTransaction, rows: &[DemoBusiness]) -> Result<u64> {
    let mut insert = Query::insert()
        .into_table(business::Entity)
        .columns([
            business::Column::Name.into_iden(),
            business::Column::Phone.into_iden(),
            business::Column::Description.into_iden(),
            business::Column::Address.into_iden(),
            Geo::Location.into_iden(),
            business::Column::IsVerified.into_iden(),
            business::Column::IsActive.into_iden(),
            business::Column::IsDemo.into_iden(),
        ])
        .to_owned();
    for b in rows {
        let location = match (b.lat, b.lng) {
            (Some(lat), Some(lng)) => geography_from_text(&point_wkt(lat, lng)),
            _ => null(),
        };
        insert
            .values([
                Expr::val(b.name.clone()).into(),
                Expr::val(b.phone.clone()).into(),
                Expr::val(b.description.clone()).into(),
                Expr::val(b.address.clone()).into(),
                location,
                Expr::val(b.is_verified).into(),
                Expr::val(true).into(),
                Expr::val(true).into(),
            ])
            .map_err(|e| Error::internal(format!("demo data businesses: {e}")))?;
    }
    txn.execute(txn.get_database_backend().build(&insert))
        .await
        .map(|r| r.rows_affected())
        .map_err(db_error("businesses"))
}

/// Demo business ids by name, and category ids by slug: what the services
/// and products refer to.
async fn references(
    txn: &DatabaseTransaction,
) -> Result<(HashMap<String, Vec<i32>>, HashMap<String, i32>)> {
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
    let categories = category::Entity::find()
        .select_only()
        .column(category::Column::Slug)
        .column(category::Column::Id)
        .into_tuple::<(String, i32)>()
        .all(txn)
        .await
        .map_err(db_error("categories read"))?
        .into_iter()
        .collect();
    Ok((businesses, categories))
}

/// Each (business id, category id) pair a demo row resolves to: none when
/// its category slug is gone.
fn resolve<'a>(
    businesses: &'a HashMap<String, Vec<i32>>,
    categories: &'a HashMap<String, i32>,
    business: &str,
    slug: &str,
) -> impl Iterator<Item = (i32, i32)> + 'a {
    let category = categories.get(slug).copied();
    businesses
        .get(business)
        .into_iter()
        .flatten()
        .filter_map(move |id| category.map(|c| (*id, c)))
}

#[async_trait]
impl DemoDataRepositoryTrait for DemoDataRepository {
    async fn count(&self) -> Result<i64> {
        business::Entity::find()
            .filter(is_demo())
            .count(self.db.as_ref())
            .await
            .map(|n| i64::try_from(n).unwrap_or(i64::MAX))
            .map_err(db_error("count"))
    }

    async fn load(&self) -> Result<DemoReport> {
        let data: DemoData = serde_json::from_str(DEMO_DATA)
            .map_err(|e| Error::internal(format!("demo data file: {e}")))?;
        // All or nothing: half-loaded demo data would make `count` say "loaded"
        // while services or products are missing.
        let txn = self.db.begin().await.map_err(db_error("begin"))?;
        let businesses = insert_businesses(&txn, &data.businesses).await?;
        let (by_name, by_slug) = references(&txn).await?;

        let services: Vec<service::ActiveModel> = data
            .services
            .iter()
            .flat_map(|s| {
                resolve(&by_name, &by_slug, &s.business, &s.category_slug).map(
                    move |(business_id, category_id)| service::ActiveModel {
                        business_id: Set(business_id),
                        category_id: Set(category_id),
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
                resolve(&by_name, &by_slug, &p.business, &p.category_slug).map(
                    move |(business_id, category_id)| product::ActiveModel {
                        business_id: Set(business_id),
                        category_id: Set(category_id),
                        name: Set(p.name.clone()),
                        description: Set(Some(p.description.clone())),
                        price: Set(Some(p.price)),
                        unit: Set(Some(p.unit.clone())),
                        ..Default::default()
                    },
                )
            })
            .collect();
        let (services_n, products_n) = (services.len() as u64, products.len() as u64);
        if !services.is_empty() {
            service::Entity::insert_many(services)
                .exec_without_returning(&txn)
                .await
                .map_err(db_error("services"))?;
        }
        if !products.is_empty() {
            product::Entity::insert_many(products)
                .exec_without_returning(&txn)
                .await
                .map_err(db_error("products"))?;
        }
        txn.commit().await.map_err(db_error("commit"))?;
        Ok(DemoReport {
            businesses,
            services: services_n,
            products: products_n,
        })
    }

    async fn remove(&self) -> Result<DemoRemoval> {
        // The cascade deletes the products, so their photos are read first,
        // in the same transaction: no photo can be added in between unseen.
        let txn = self.db.begin().await.map_err(db_error("begin"))?;
        let mut photos = product::Entity::find()
            .select_only()
            .column(product::Column::ImageFileId)
            .join(JoinType::InnerJoin, product::Relation::Business.def())
            .filter(is_demo())
            .filter(product::Column::ImageFileId.is_not_null());
        // FOR UPDATE OF products: the business rows stay unlocked.
        QueryTrait::query(&mut photos).lock_with_tables(LockType::Update, [product::Entity]);
        let image_files: Vec<Uuid> = photos
            .into_tuple::<Uuid>()
            .all(&txn)
            .await
            .map_err(db_error("remove images"))?;
        let businesses = business::Entity::delete_many()
            .filter(is_demo())
            .exec(&txn)
            .await
            .map(|r| r.rows_affected)
            .map_err(db_error("remove"))?;
        txn.commit().await.map_err(db_error("commit"))?;
        Ok(DemoRemoval {
            businesses,
            image_files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_demo_data_file_parses_and_refers_to_its_own_businesses() {
        let data: std::result::Result<DemoData, _> = serde_json::from_str(DEMO_DATA);
        assert!(data.is_ok(), "{:?}", data.err());
        let data = data.unwrap_or_else(|_| DemoData {
            businesses: vec![],
            services: vec![],
            products: vec![],
        });
        let names: Vec<&str> = data.businesses.iter().map(|b| b.name.as_str()).collect();
        for named in data
            .services
            .iter()
            .map(|s| s.business.as_str())
            .chain(data.products.iter().map(|p| p.business.as_str()))
        {
            assert!(names.contains(&named), "{named} is not a demo business");
        }
    }
}
