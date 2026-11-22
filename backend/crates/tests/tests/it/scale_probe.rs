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
use domain::models::admin_list::{AdminFilter, AdminSort};
use domain::models::{
    business::ProviderSort,
    product::{AdminProductFilter, ProductFilter, ProductSort},
};
use domain::{
    models::demand::DemoScope, repositories::demand_repository_trait::DemandRepositoryTrait,
};
use infrastructure::repositories::{
    admin_unit_repository::AdminUnitRepository, demand_repository::DemandRepository,
};
use integration_tests::{
    body_text, env_with_origin, first_admin, get_page, load_admin_units, send, sign_in, Env,
};
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
    let e = env_with_origin().await;
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
    load(
        &e,
        "a partner over the busiest province",
        "INSERT INTO partners (name, kind) VALUES ('Dai ly drone', 'dealer');
         INSERT INTO partner_contracts (partner_id, province_code, starts_on, ends_on, monthly_fee)
         SELECT (SELECT max(id) FROM partners), w.province_code, current_date,
                current_date + 365, 1000000
         FROM businesses b JOIN wards w ON w.code = b.ward_code
         GROUP BY w.province_code ORDER BY count(*) DESC LIMIT 1",
    )
    .await;
    load(
        &e,
        "500 more partners, 3 000 contracts over wards, 3 types and 2 shops each",
        "INSERT INTO partners (name, kind) SELECT 'Doi tac ' || n, 'brand' FROM generate_series(1, 500) AS n;
         INSERT INTO partner_contracts (partner_id, province_code, ward_code, starts_on, ends_on, monthly_fee)
         SELECT p.id, w.province_code, w.code, current_date - 30, current_date + 300, 500000 + k * 1000
         FROM (SELECT id, row_number() OVER (ORDER BY id) AS r FROM partners WHERE name LIKE 'Doi tac %') p,
              generate_series(0, 5) AS k,
              LATERAL (SELECT code, province_code FROM wards
                       WHERE code IN (SELECT ward_code FROM businesses WHERE ward_code IS NOT NULL)
                       ORDER BY code OFFSET ((p.r * 7 + k * 13) % 300) LIMIT 1) w;
         INSERT INTO partner_contract_service_types (contract_id, service_type_id)
         SELECT c.id, t.id FROM partner_contracts c,
                LATERAL (SELECT id FROM service_types ORDER BY id LIMIT 3) t
         WHERE c.ward_code IS NOT NULL;
         INSERT INTO partner_sponsored (contract_id, business_id)
         SELECT c.id, b.id FROM partner_contracts c,
                LATERAL (SELECT id FROM businesses WHERE ward_code = c.ward_code ORDER BY id LIMIT 2) b
         WHERE c.ward_code IS NOT NULL",
    )
    .await;
    load(&e, "statistics", "ANALYZE").await;
    let contract =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "SELECT partner_id, id FROM partner_contracts WHERE ward_code IS NULL LIMIT 1",
            ))
            .await
            .unwrap()
            .unwrap();
    let partner_id: i32 = contract.try_get("", "partner_id").unwrap();
    let contract_id: i32 = contract.try_get("", "id").unwrap();

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
        "admin: demand report, one province",
        u.demand.report(
            &admin,
            Some(application::admin_units::AreaKey::Province("82".into()))
        )
    );
    timed!(
        "admin: partners, page 1",
        u.partner.list_partners(&admin, 1, &AdminFilter::default())
    );
    timed!("admin: partners, revenue by area", u.partner.kpis(&admin));
    timed!(
        "admin: one partner",
        u.partner.get_partner(&admin, partner_id)
    );
    timed!(
        "admin: sponsor search, whole province",
        u.partner
            .sponsor_candidates(&admin, partner_id, contract_id, None)
    );
    timed!(
        "admin: sponsor search, a name",
        u.partner
            .sponsor_candidates(&admin, partner_id, contract_id, Some("cap 19999"))
    );
    timed!(
        "admin: providers, page 1",
        u.admin.list_providers(&admin, 1, &AdminFilter::default())
    );
    timed!(
        "admin: providers, last page",
        u.admin.list_providers(&admin, 500, &AdminFilter::default())
    );
    let by_name = AdminFilter {
        search: Some("cap 19999".into()),
        ..AdminFilter::default()
    };
    timed!(
        "admin: providers by phone",
        u.admin.list_providers(
            &admin,
            1,
            &AdminFilter {
                search: Some("0912".into()),
                ..AdminFilter::default()
            }
        )
    );
    timed!(
        "admin: contact requests by provider name, oldest first",
        u.admin.list_inquiries(
            &admin,
            1,
            &AdminFilter {
                sort: Some(AdminSort::Oldest),
                ..by_name.clone()
            }
        )
    );
    timed!(
        "admin: services by provider name",
        u.admin.list_services_admin(&admin, 1, &by_name)
    );
    let supplies = AdminProductFilter {
        provider_id: None,
        category_id: None,
        commodity_id: None,
        search: Some("cap 19999".into()),
        ..AdminProductFilter::default()
    };
    timed!(
        "admin: supplies by provider name",
        u.admin.list_products_admin(&admin, 1, &supplies)
    );

    // The demand report's reads one by one: which of them costs the time.
    let db = std::sync::Arc::new(e.t.db.clone());
    let demand = DemandRepository::new(db.clone());
    let units = AdminUnitRepository::new(db);
    let to = chrono::Utc::now();
    let from = to - chrono::Duration::weeks(8);
    timed!(
        "demand: lead counts",
        demand.lead_counts(from, to, DemoScope::All)
    );
    timed!(
        "demand: search counts",
        demand.search_counts(from, to, DemoScope::All)
    );
    timed!("demand: any demo business", demand.any_demo_business());
    timed!("demand: sole categories", demand.sole_categories());
    timed!(
        "demand: admin units",
        application::admin_units::AdminUnits::load(&units)
    );

    // The same reads written out, to see where Postgres spends the time.
    let week =
        "floor(date_part('epoch', {t}.created_on - (now() - interval '8 weeks')) / 604800)::int4";
    explain(
        &e,
        "demand: taps by week, ward, category, channel",
        &format!(
            "SELECT {w} AS week, CASE WHEN b.deleted_at IS NULL THEN b.ward_code END AS area,
                    COALESCE(s.service_type_id, p.supply_type_id) AS category_id,
                    CASE WHEN COALESCE(s.service_type_id, p.supply_type_id) IS NULL THEN c.business_id END AS shop_id,
                    c.channel, count(c.id)
             FROM call_clicks c JOIN businesses b ON b.id = c.business_id
             LEFT JOIN services s ON s.id = c.service_id LEFT JOIN products p ON p.id = c.product_id
             WHERE c.created_on >= now() - interval '8 weeks' AND c.created_on < now()
             GROUP BY 1, 2, 3, 4, 5",
            w = week.replace("{t}", "c")
        ),
    )
    .await;
    explain(
        &e,
        "demand: contact requests by week, ward, category, phone",
        &format!(
            "SELECT {w} AS week, CASE WHEN b.deleted_at IS NULL THEN b.ward_code END AS area,
                    COALESCE(s.service_type_id, p.supply_type_id) AS category_id,
                    i.requester_phone, count(i.id)
             FROM inquiries i JOIN businesses b ON b.id = i.business_id
             LEFT JOIN services s ON s.id = i.service_id LEFT JOIN products p ON p.id = i.product_id
             WHERE i.created_on >= now() - interval '8 weeks' AND i.created_on < now()
               AND i.status <> 'spam'
             GROUP BY 1, 2, 3, 4",
            w = week.replace("{t}", "i")
        ),
    )
    .await;
    explain(
        &e,
        "demand: searches",
        &format!(
            "SELECT {w} AS week, CASE WHEN results = 0 THEN query END AS words,
                    service_type_id, supply_type_id, ward_code, results = 0 AS unmet, count(id)
             FROM search_events
             WHERE created_on >= now() - interval '8 weeks' AND created_on < now()
             GROUP BY 1, 2, 3, 4, 5, 6",
            w = week.replace("{t}", "search_events")
        ),
    )
    .await;

    // Whole pages, as the browser gets them: the reads and the rendering.
    let cookie = sign_in(&e, "quantri", "matkhau123").await;
    for path in [
        "/admin/demand".to_owned(),
        "/admin/demand?area=p82".to_owned(),
        "/admin/partners".to_owned(),
        format!("/admin/partners/{partner_id}"),
        format!("/admin/partners/{partner_id}?tab=contracts"),
        format!("/admin/partners/{partner_id}/contracts/{contract_id}"),
    ] {
        let started = Instant::now();
        let html = body_text(send(&e.router, get_page(&path, Some(&cookie))).await).await;
        println!(
            "page {:<39} {:>10.1?} {:>8} KB",
            path,
            started.elapsed(),
            html.len() / 1024
        );
    }

    explain(
        &e,
        "the taps of thirty days (the dashboard's count)",
        "SELECT count(*) FROM call_clicks WHERE created_on >= now() - interval '30 days'",
    )
    .await;
    e.cleanup().await;
}
