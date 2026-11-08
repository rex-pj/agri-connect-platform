//! Accent-insensitive search against a real Postgres (unaccent + pg_trgm).
//! Run with TEST_DATABASE_URL and `--include-ignored`.

use std::sync::Arc;

use application::{catalogue::CATALOGUE, inputs::BrowseParams};
use domain::{
    models::{
        business::ProviderSort,
        product::{ProductFilter, ProductSort},
    },
    repositories::catalogue_repository_trait::CatalogueRepositoryTrait,
};
use infrastructure::{
    catalogue_repository::CatalogueRepository,
    search::{contains, contains_pattern, SearchColumn},
};
use integration_tests::{state, TestDb};
use migration::search::all_fold_indexes;
use sea_orm::sea_query::{ColumnRef, Expr, Query};
use sea_orm::{ConnectionTrait, Statement, TransactionTrait};

async fn seeded() -> TestDb {
    let t = TestDb::create().await;
    CatalogueRepository::new(Arc::new(t.db.clone()))
        .apply(&CATALOGUE)
        .await
        .unwrap();
    let insert = |sql: &str, values: Vec<sea_orm::Value>| {
        Statement::from_sql_and_values(t.db.get_database_backend(), sql, values)
    };
    for (name, description) in [
        ("Đại lý Phân bón Đồng Tháp", "Phân NPK, phân hữu cơ"),
        ("Dịch vụ Múc bùn Cái Lậy", "Nạo vét kênh mương"),
        ("Cửa hàng 50% giảm giá", "Khuyến mãi"),
    ] {
        t.db.execute(insert(
            "INSERT INTO businesses (name, phone, description) VALUES ($1, '0901234567', $2)",
            vec![name.into(), description.into()],
        ))
        .await
        .unwrap();
    }
    // The directory lists only businesses offering at least one active service.
    t.db.execute_unprepared(
        "INSERT INTO services (business_id, category_id, name)
         SELECT b.id, c.id, 'Dịch vụ' FROM businesses b,
           (SELECT id FROM categories WHERE type = 'service' LIMIT 1) c",
    )
    .await
    .unwrap();
    t.db.execute(insert(
        "INSERT INTO products (business_id, category_id, name, description, price, unit)
         SELECT b.id, c.id, $1, $2, 420000, 'bao' FROM businesses b, categories c
         WHERE b.name LIKE 'Đại lý%' AND c.slug = 'phan-bon'",
        vec!["Phân Urê Phú Mỹ".into(), "Đạm 46%, bao 50 kg".into()],
    ))
    .await
    .unwrap();
    t
}

async fn providers(t: &TestDb, query: &str) -> Vec<String> {
    state(t.db.clone())
        .usecases
        .explore
        .browse_providers(BrowseParams {
            service_id: None,
            name: Some(query.into()),
            sort: ProviderSort::default(),
            page: 1,
        })
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|b| b.name)
        .collect()
}

async fn products(t: &TestDb, query: &str) -> Vec<String> {
    state(t.db.clone())
        .usecases
        .product
        .browse_products(
            ProductFilter {
                name: Some(query.into()),
                ..Default::default()
            },
            ProductSort::Newest,
            1,
        )
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|p| p.name)
        .collect()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn typing_without_diacritics_finds_vietnamese_listings() {
    let t = seeded().await;
    assert_eq!(
        providers(&t, "phan bon").await,
        ["Đại lý Phân bón Đồng Tháp"]
    );
    // đ/Đ is not a combining accent; unaccent must still map it to d.
    assert_eq!(
        providers(&t, "dong thap").await,
        ["Đại lý Phân bón Đồng Tháp"]
    );
    assert_eq!(providers(&t, "MUC BUN").await, ["Dịch vụ Múc bùn Cái Lậy"]);
    // A provider is found by its services too: every one offers "Dịch vụ".
    assert_eq!(providers(&t, "DICH VU").await.len(), 3);
    // Description matches count too.
    assert_eq!(providers(&t, "nao vet").await, ["Dịch vụ Múc bùn Cái Lậy"]);
    assert_eq!(products(&t, "phan ure").await, ["Phân Urê Phú Mỹ"]);
    assert_eq!(products(&t, "dam 46").await, ["Phân Urê Phú Mỹ"]);
    t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_accented_query_still_works() {
    let t = seeded().await;
    assert_eq!(
        providers(&t, "Phân bón").await,
        ["Đại lý Phân bón Đồng Tháp"]
    );
    t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_percent_sign_is_a_literal_not_a_wildcard() {
    let t = seeded().await;
    assert_eq!(providers(&t, "50%").await, ["Cửa hàng 50% giảm giá"]);
    // Unescaped, "%" alone would match every row.
    assert_eq!(providers(&t, "%").await, ["Cửa hàng 50% giảm giá"]);
    t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_search_query_uses_each_folded_index() {
    // If the query expression stops matching the index expression character
    // for character, Postgres silently falls back to a full scan; this is the
    // test that notices.
    let t = seeded().await;
    let indexes = all_fold_indexes();
    for column in [
        SearchColumn::BusinessName,
        SearchColumn::BusinessDescription,
        SearchColumn::ProductName,
        SearchColumn::ProductDescription,
        SearchColumn::ServiceName,
        SearchColumn::ServiceDescription,
    ] {
        let ColumnRef::TableColumn(table, name) = column.column_ref() else {
            unreachable!("search columns are table-qualified")
        };
        let index = indexes
            .iter()
            .find(|i| {
                i.table.to_string() == table.to_string() && i.column.to_string() == name.to_string()
            })
            .map(|i| i.name)
            .unwrap();
        let txn = t.db.begin().await.unwrap();
        // Tiny test tables would make a seq scan cheaper; we are asking whether
        // the index CAN be used, not whether it is worth it here.
        txn.execute_unprepared("SET LOCAL enable_seqscan = off")
            .await
            .unwrap();
        // The very condition the repositories build.
        let query = Query::select()
            .expr(Expr::val(1))
            .from(table.clone())
            .and_where(contains(column, &contains_pattern("phan bon")))
            .to_owned();
        let mut explain = txn.get_database_backend().build(&query);
        explain.sql = format!("EXPLAIN {}", explain.sql);
        let plan: Vec<String> = txn
            .query_all(explain)
            .await
            .unwrap()
            .iter()
            .map(|row| row.try_get::<String>("", "QUERY PLAN").unwrap())
            .collect();
        txn.rollback().await.unwrap();
        assert!(
            plan.iter().any(|l| l.contains(index)),
            "{column:?}: plan does not use {index}:\n{}",
            plan.join("\n")
        );
    }
    t.cleanup().await;
}
