//! The demo data agrees with itself, read back through the app's own read
//! paths: the counts match the file, the admin's demand report counts the
//! demo leads and searches (a real partner's never would), the partners'
//! revenue is the contracts in force today, every sponsored shop lies in
//! its contract's area, reviews come from answered requests, the price
//! board shows the fresh prices only. Removing it leaves nothing behind.
//! Database tests: TEST_DATABASE_URL and `--include-ignored`.

use std::sync::Arc;

use chrono::{Duration, Utc};
use domain::{
    models::{catalogue::DemoVolume, demand::DemoScope},
    repositories::{
        demand_repository_trait::DemandRepositoryTrait,
        demo_data_repository_trait::DemoDataRepositoryTrait,
    },
    ErrorCode,
};
use infrastructure::{
    demo_data_repository::DemoDataRepository, repositories::demand_repository::DemandRepository,
};
use integration_tests::{first_admin, load_admin_units, Env};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde_json::Value;

const DATA: &str = include_str!("../../../infrastructure/src/demo/data.json");

async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
    db.query_one(Statement::from_string(db.get_database_backend(), sql))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "n")
        .unwrap()
}

/// The number of rows a list of `key` arrays in the file stands for.
fn weekly_total(data: &Value, list: &str) -> u64 {
    data[list]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|item| item["weekly"].as_array().unwrap())
        .map(|n| n.as_u64().unwrap())
        .sum()
}

fn len(data: &Value, list: &str) -> u64 {
    data[list].as_array().unwrap().len() as u64
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn demo_data_agrees_with_itself_and_leaves_nothing_behind() {
    let e = Env::new().await;
    load_admin_units(&e).await;
    let admin = first_admin(&e.state).await;
    let data: Value = serde_json::from_str(DATA).unwrap();

    // Every row of the file is loaded: no reference dropped one.
    let report = e.state.usecases.setup.load_demo(&admin).await.unwrap();
    let contracts: u64 = data["partners"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["contracts"].as_array().unwrap().len() as u64)
        .sum();
    let prices: u64 = data["prices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["points"].as_array().unwrap().len() as u64)
        .sum();
    assert_eq!(report.businesses, len(&data, "businesses"));
    assert_eq!(report.services, len(&data, "services"));
    assert_eq!(report.products, len(&data, "products"));
    assert_eq!(report.inquiries, len(&data, "inquiries"));
    assert_eq!(report.reviews, len(&data, "reviews"));
    assert_eq!(report.calls, weekly_total(&data, "calls"));
    assert_eq!(report.searches, weekly_total(&data, "searches"));
    assert_eq!(report.prices, prices);
    assert_eq!(report.jobs, len(&data, "jobs"));
    assert_eq!(report.partners, len(&data, "partners"));
    assert_eq!(report.contracts, contracts);
    assert_eq!(report.alerts, len(&data, "alerts"));
    let demo = DemoDataRepository::new(Arc::new(e.t.db.clone()));
    let present = demo.count().await.unwrap();
    assert_eq!(present.calls, report.calls);
    assert_eq!(present.contracts, report.contracts);
    assert_eq!(present.alerts, report.alerts);
    // A demo alert over a shop's province is placed there.
    assert_eq!(
        count(
            &e.t.db,
            "SELECT COUNT(*) AS n FROM alerts WHERE is_demo AND province_code IS NOT NULL"
        )
        .await,
        2
    );

    // Every sponsored shop lies in its contract's area, as the admin's form
    // requires: all four in the file made it.
    assert_eq!(
        count(&e.t.db, "SELECT COUNT(*) AS n FROM partner_sponsored").await,
        3
    );
    // Reviews only from the phone of a request the shop answered.
    assert_eq!(
        count(
            &e.t.db,
            "SELECT COUNT(*) AS n FROM reviews r JOIN inquiries i ON i.id = r.inquiry_id
             WHERE i.business_id = r.business_id AND i.status IN ('contacted', 'closed')"
        )
        .await,
        report.reviews as i64
    );

    // The admin's report counts the demo leads and says so; Đồng Tháp has
    // 20 leads or more every week; the unanswered searches show.
    let demand = e.state.usecases.demand.report(&admin, None).await.unwrap();
    assert!(demand.with_demo);
    assert!(demand.steady_areas >= 1, "Đồng Tháp is steady");
    assert!(demand.all.total.value.unwrap_or(0) > 0);
    assert!(
        demand.unmet.iter().any(|u| u.query == "may cay"),
        "{:?}",
        demand.unmet.iter().map(|u| &u.query).collect::<Vec<_>>()
    );
    // A real partner's report never reads them; a demo partner's only them.
    let repo = DemandRepository::new(Arc::new(e.t.db.clone()));
    let (from, to) = (Utc::now() - Duration::weeks(10), Utc::now());
    assert!(repo
        .lead_counts(from, to, DemoScope::Real)
        .await
        .unwrap()
        .is_empty());
    assert!(repo
        .search_counts(from, to, DemoScope::Real)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        repo.lead_counts(from, to, DemoScope::Demo)
            .await
            .unwrap()
            .len(),
        repo.lead_counts(from, to, DemoScope::All)
            .await
            .unwrap()
            .len()
    );

    // Revenue: the contracts in force today (not the expired one, not the
    // one starting later).
    let kpis = e.state.usecases.partner.kpis(&admin).await.unwrap();
    assert_eq!(kpis.paying_partners, 3);
    assert_eq!(kpis.monthly_revenue, 15_000_000);

    // The price board: fresh prices with their move; a stale one left out.
    let board = e.state.usecases.price.board(None, None).await.unwrap();
    let quote = |crop: &str| {
        board
            .crops
            .iter()
            .find(|c| c.name == crop)
            .and_then(|c| c.quotes.first())
            .cloned()
    };
    let om18 = quote("Lúa OM 18").unwrap();
    assert_eq!((om18.price_low, om18.price_high), (7200, 7400));
    assert_eq!(om18.change, Some(200), "7 300 against 7 100");
    assert!(quote("Thanh long ruột đỏ").is_none(), "20 days old: stale");

    // Reviews: shown ones only, with the provider's reply.
    let binh = count(
        &e.t.db,
        "SELECT id::bigint AS n FROM businesses WHERE name = 'Dịch vụ Trần Thị Bình'",
    )
    .await;
    let reviews = e
        .state
        .usecases
        .review
        .list_public(binh as i32)
        .await
        .unwrap();
    assert_eq!(reviews.count, 2);
    assert_eq!(reviews.average, Some(4.5));

    // Removing leaves no demo row, and the platform data whole.
    e.state.usecases.setup.remove_demo(&admin).await.unwrap();
    let left = demo.count().await.unwrap();
    assert!(!left.any(), "{left:?}");
    for table in [
        "businesses",
        "inquiries",
        "reviews",
        "call_clicks",
        "search_events",
        "crop_prices",
        "job_posts",
        "alerts",
        "partners",
        "partner_contracts",
        "partner_sponsored",
        "commodities",
        "service_types",
        "supply_types",
        "locations",
        "crops",
    ] {
        let n = count(&e.t.db, &format!("SELECT COUNT(*) AS n FROM {table}")).await;
        assert_eq!(n, 0, "{table}");
    }
    assert!(count(&e.t.db, "SELECT COUNT(*) AS n FROM wards").await > 3000);
    e.cleanup().await;
}

/// The demo data at volume (`agri-connect demo-data`) keeps the same rules
/// as the file's, at a size where a rule broken one time in a hundred shows:
/// every review from an answered request of its shop, every sponsored shop
/// in its contract's area, every shop managed by a demo provider account.
/// A second load is refused; removing it takes the accounts too.
#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn demo_data_at_volume_keeps_the_rules_and_leaves_nothing_behind() {
    let e = Env::new().await;
    load_admin_units(&e).await;
    let admin = first_admin(&e.state).await;
    let volume = DemoVolume {
        providers: 150,
        seed: 11,
    };
    let password = "Demo-volume-pass-2026";
    let setup = &e.state.usecases.setup;
    let report = setup.load_demo_volume(&volume, password).await.unwrap();
    assert!(report.businesses >= 140, "{report:?}");
    assert!(report.accounts >= 130, "{report:?}");
    assert!(
        report.services >= 140 && report.products >= 70,
        "{report:?}"
    );
    assert!(
        report.inquiries >= 250 && report.reviews >= 100,
        "{report:?}"
    );
    assert!(report.jobs >= 150 && report.alerts >= 150, "{report:?}");
    assert!(report.partners >= 6 && report.contracts >= 6, "{report:?}");
    assert!(report.prices >= 1_000 && report.calls > 0, "{report:?}");
    let present = DemoDataRepository::new(Arc::new(e.t.db.clone()))
        .count()
        .await
        .unwrap();
    assert_eq!(present.accounts, report.accounts);
    assert_eq!(present.reviews, report.reviews);

    // Nothing the generator wrote was dropped by a reference it got wrong.
    let file: Value = serde_json::from_str(DATA).unwrap();
    assert!(report.jobs >= len(&file, "jobs") + u64::from(volume.providers));
    assert_eq!(
        count(
            &e.t.db,
            "SELECT COUNT(*) AS n FROM reviews r JOIN inquiries i ON i.id = r.inquiry_id
             WHERE i.business_id = r.business_id AND i.status IN ('contacted', 'closed')"
        )
        .await,
        report.reviews as i64
    );
    assert_eq!(
        count(
            &e.t.db,
            "SELECT COUNT(*) AS n FROM partner_sponsored s
             JOIN partner_contracts c ON c.id = s.contract_id
             JOIN businesses b ON b.id = s.business_id
             JOIN wards w ON w.code = b.ward_code
             WHERE (c.ward_code IS NOT NULL AND c.ward_code <> b.ward_code)
                OR (c.ward_code IS NULL AND c.province_code <> w.province_code)"
        )
        .await,
        0,
        "a sponsored shop outside its contract's area"
    );
    assert_eq!(
        count(
            &e.t.db,
            "SELECT COUNT(*) AS n FROM businesses b JOIN users u ON u.id = b.owner_id
             JOIN roles r ON r.id = u.role_id
             WHERE b.is_demo AND (NOT u.is_demo OR r.code <> 'provider')"
        )
        .await,
        0,
        "a demo shop managed by anything but a demo provider account"
    );
    assert!(
        count(
            &e.t.db,
            "SELECT COUNT(*) AS n FROM businesses WHERE is_demo AND ward_code IS NOT NULL"
        )
        .await
            >= 130,
        "shops are placed in wards"
    );

    // The pages read it: the price board, the admin's report.
    let board = e.state.usecases.price.board(None, None).await.unwrap();
    assert!(
        board.crops.len() > 20,
        "{} crops on the board",
        board.crops.len()
    );
    let demand = e.state.usecases.demand.report(&admin, None).await.unwrap();
    // The total is shown, or hidden with what it is at least (it would give
    // one small hidden figure away): leads are counted either way.
    let total = &demand.all.total;
    assert!(demand.with_demo && total.value.or(total.at_least).unwrap_or(0) > 0);

    // Once is enough: a second load would double it.
    let again = setup.load_demo_volume(&volume, password).await.unwrap_err();
    assert_eq!(again.code(), ErrorCode::DemoAlreadyLoaded);

    setup.remove_demo(&admin).await.unwrap();
    for table in [
        "businesses",
        "partners",
        "job_posts",
        "alerts",
        "crop_prices",
    ] {
        let n = count(&e.t.db, &format!("SELECT COUNT(*) AS n FROM {table}")).await;
        assert_eq!(n, 0, "{table}");
    }
    assert_eq!(
        count(&e.t.db, "SELECT COUNT(*) AS n FROM users WHERE is_demo").await,
        0
    );
    assert!(
        count(&e.t.db, "SELECT COUNT(*) AS n FROM users").await >= 1,
        "the admin stays"
    );
    e.cleanup().await;
}
