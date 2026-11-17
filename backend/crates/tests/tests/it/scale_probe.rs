//! A measurement, not a check: the main reads timed against a large database
//! (20 000 providers, 60 000 services, 100 000 supplies, a million taps),
//! to decide an index or a rewrite from numbers. It takes a couple of
//! minutes, so it runs only when asked:
//!
//! ```text
//! SCALE_PROBE=1 TEST_DATABASE_URL=postgres://... \
//!   cargo test -p integration-tests --test it scale_probe -- --include-ignored --nocapture
//! ```
//!
//! Read each line against the last run's (noted in CLAUDE.md, "Scale"): a
//! read several times slower than it was is the one to look at.

use std::time::Instant;

use application::inputs::BrowseParams;
use domain::models::{
    business::ProviderSort,
    product::{AdminProductFilter, ProductFilter, ProductSort},
};
use integration_tests::{first_admin, load_admin_units, Env};
use sea_orm::{ConnectionTrait, Statement};

/// Set to run the probe.
const SWITCH: &str = "SCALE_PROBE";

async fn load(e: &Env, what: &str, sql: &str) {
    let started = Instant::now();
    e.t.db.execute_unprepared(sql).await.unwrap();
    println!("  loaded {what} in {:.1?}", started.elapsed());
}

/// Prints how Postgres runs `sql`.
async fn explain(e: &Env, what: &str, sql: &str) {
    let rows =
        e.t.db
            .query_all(Statement::from_string(
                e.t.db.get_database_backend(),
                format!("EXPLAIN (ANALYZE, BUFFERS) {sql}"),
            ))
            .await
            .unwrap();
    println!("--- {what}");
    for row in rows {
        let line: String = row.try_get("", "QUERY PLAN").unwrap();
        println!("    {line}");
    }
}

/// Runs a read twice and prints both times: the second is the steady one.
macro_rules! timed {
    ($what:expr, $read:expr) => {{
        let started = Instant::now();
        $read.await.unwrap();
        let first = started.elapsed();
        let started = Instant::now();
        $read.await.unwrap();
        println!(
            "{:<44} {:>10.1?} {:>10.1?}",
            $what,
            first,
            started.elapsed()
        );
    }};
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_main_reads_against_a_large_database() {
    if std::env::var_os(SWITCH).is_none() {
        return;
    }
    let e = Env::new().await;
    load_admin_units(&e).await;
    let admin = first_admin(&e.state).await;

    load(
        &e,
        "20 000 providers across the delta",
        "INSERT INTO businesses (name, phone, description, address, location, is_verified)
         SELECT 'Nha cung cap ' || n, '09' || lpad(n::text, 8, '0'),
                'Chuyen phun thuoc, muc bun, sua may nong nghiep so ' || n, 'Ap ' || n,
                ST_SetSRID(ST_MakePoint(105.2 + random() * 1.2, 9.6 + random() * 1.1), 4326)::geography,
                n % 5 = 0
         FROM generate_series(1, 20000) AS n",
    )
    .await;
    load(
        &e,
        "their wards",
        "UPDATE businesses SET ward_code = (SELECT code FROM wards WHERE is_active
           AND ST_Covers(boundary, businesses.location) ORDER BY code LIMIT 1)",
    )
    .await;
    load(
        &e,
        "60 000 services",
        "INSERT INTO services (business_id, service_type_id, name, description, created_on)
         SELECT b.id,
                (SELECT (array_agg(id ORDER BY id))[1 + (b.id + k) % count(*)] FROM service_types),
                CASE k WHEN 0 THEN 'Phun thuoc drone ' WHEN 1 THEN 'Muc bun vuon '
                       ELSE 'Sua may cay ' END || b.id,
                'Nhan lam tan noi, gia thoa thuan, lien he truoc mot ngay. Ma ' || b.id,
                now() - (random() * interval '300 days')
         FROM businesses b, generate_series(0, 2) AS k",
    )
    .await;
    load(
        &e,
        "100 000 supplies",
        "INSERT INTO products (business_id, supply_type_id, name, description, price, created_on)
         SELECT b.id,
                (SELECT (array_agg(id ORDER BY id))[1 + (b.id + k) % count(*)] FROM supply_types),
                'Phan bon NPK loai ' || k || ' ' || b.id, 'Bao 50kg, giao tan vuon',
                10000 + k * 500, now() - (random() * interval '300 days')
         FROM businesses b, generate_series(0, 4) AS k",
    )
    .await;
    load(
        &e,
        "a million taps over eight weeks",
        "INSERT INTO call_clicks (business_id, service_id, channel, created_on)
         SELECT s.business_id, s.id, CASE WHEN k % 4 = 0 THEN 'zalo' ELSE 'call' END,
                now() - ((k * 3.2 + random()) * interval '1 day')
         FROM services s, generate_series(0, 16) AS k",
    )
    .await;
    load(
        &e,
        "200 000 searches",
        "INSERT INTO search_events (query, results, created_on)
         SELECT 'may cay ' || (n % 500), n % 3, now() - (random() * interval '56 days')
         FROM generate_series(1, 200000) AS n",
    )
    .await;
    load(
        &e,
        "100 000 contact requests",
        "INSERT INTO inquiries (business_id, topic, requester_name, requester_phone, created_on)
         SELECT b.id, 'Phun thuoc', 'Chu Tu', '09' || lpad((b.id * 7 + k)::text, 8, '0'),
                now() - (random() * interval '56 days')
         FROM businesses b, generate_series(0, 4) AS k",
    )
    .await;
    load(
        &e,
        "sixty days of prices for every crop and area",
        "INSERT INTO crop_prices (crop_id, location_id, observed_on, price_low, price_high)
         SELECT c.id, l.id, current_date - d, 10000 + d, 12000 + d
         FROM crops c, locations l, generate_series(0, 59) AS d",
    )
    .await;
    load(&e, "statistics", "ANALYZE").await;

    let u = &e.state.usecases;
    let list = |page: i64, words: Option<&str>, sort: ProviderSort| BrowseParams {
        service_id: None,
        sector: None,
        name: words.map(str::to_owned),
        sort,
        page,
    };
    let by_turn = ProviderSort::VerifiedFirst;
    let newest = ProviderSort::Newest;
    println!("\n{:<44} {:>10} {:>10}", "read", "first", "again");
    timed!(
        "/services, default order, page 1",
        u.explore.browse_services(list(1, None, by_turn))
    );
    timed!(
        "/services, default order, page 50",
        u.explore.browse_services(list(50, None, by_turn))
    );
    timed!(
        "/services, newest, page 1",
        u.explore.browse_services(list(1, None, newest))
    );
    timed!(
        "/services, words in the service",
        u.explore.browse_services(list(1, Some("muc bun"), by_turn))
    );
    timed!(
        "/services, words in the provider's name",
        u.explore
            .browse_services(list(1, Some("cap 19999"), by_turn))
    );
    timed!(
        "/providers, page 1",
        u.explore.browse_providers(list(1, None, by_turn))
    );
    timed!(
        "/providers, words in its services",
        u.explore
            .browse_providers(list(1, Some("sua may cay"), by_turn))
    );
    timed!(
        "/supplies, newest, page 1",
        u.product
            .browse_products(ProductFilter::default(), ProductSort::Newest, 1)
    );
    timed!(
        "/supplies, newest, page 50",
        u.product
            .browse_products(ProductFilter::default(), ProductSort::Newest, 50)
    );
    timed!(
        "/supplies, words",
        u.product.browse_products(
            ProductFilter {
                name: Some("npk loai 3".into()),
                ..ProductFilter::default()
            },
            ProductSort::Newest,
            1
        )
    );
    timed!("home: featured rows", u.explore.get_explore_data(None));
    timed!("home: price board", u.price.board(None, None));
    timed!("admin: demand report", u.demand.report(&admin, None));
    timed!("admin: dashboard figures", u.stats.get_stats(&admin));
    timed!(
        "admin: providers, page 1",
        u.admin.list_providers(&admin, 1, None)
    );
    timed!(
        "admin: providers, last page",
        u.admin.list_providers(&admin, 500, None)
    );
    timed!(
        "admin: services by provider name",
        u.admin
            .list_services_admin(&admin, 1, None, Some("cap 19999"))
    );
    let supplies = AdminProductFilter {
        provider_id: None,
        category_id: None,
        commodity_id: None,
        search: Some("cap 19999".into()),
    };
    timed!(
        "admin: supplies by provider name",
        u.admin.list_products_admin(&admin, 1, &supplies)
    );

    explain(
        &e,
        "the taps of thirty days (the dashboard's count)",
        "SELECT count(*) FROM call_clicks WHERE created_on >= now() - interval '30 days'",
    )
    .await;
    e.cleanup().await;
}
