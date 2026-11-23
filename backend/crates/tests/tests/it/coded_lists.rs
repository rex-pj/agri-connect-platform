//! The short lists the admin keeps by code: the alert kinds are data, a tab
//! of the alerts page, and an alert takes one of them (its icon reaches the
//! home page); the admin's supply and service lists say each row's type and
//! sector. (The sectors themselves: `sectors.rs`.)
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use chrono::{Duration, Utc};
use integration_tests::{
    body_text, env_with_origin, first_admin, get_page, install_admin, post_form, send, sign_in, Env,
};
use sea_orm::{ConnectionTrait, Statement};

async fn page(e: &Env, path: &str, cookie: Option<&str>) -> String {
    let res = send(&e.router, get_page(path, cookie)).await;
    let status = res.status();
    if status != StatusCode::OK {
        panic!("{path}: {status}: {}", body_text(res).await);
    }
    body_text(res).await
}

async fn scalar(e: &Env, sql: &str) -> String {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            sql.to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "v")
        .unwrap()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_alert_kinds_are_data_on_a_tab_of_the_alerts_page() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    // The second tab; the former address moves there.
    let alerts = page(&e, "/admin/alerts", Some(&admin)).await;
    assert!(alerts.contains(r#"href="/admin/alerts?tab=kinds""#));
    let kinds = page(&e, "/admin/alerts?tab=kinds", Some(&admin)).await;
    assert!(kinds.contains("Dịch bệnh") && kinds.contains("<code>salinity</code>"));
    assert!(kinds.contains(r#"href="/admin/alerts?tab=kinds" aria-current="page""#));
    let res = send(&e.router, get_page("/admin/alert-kinds", Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(res.headers()[header::LOCATION], "/admin/alerts?tab=kinds");

    // A new kind: its code from its name, its own page.
    let res = send(
        &e.router,
        post_form(
            "/admin/alert-kinds",
            &[
                ("name", "Sâu bệnh hại"),
                ("icon", "fa-bug"),
                ("display_order", "4"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let id = scalar(
        &e,
        "SELECT id::text AS v FROM alert_kinds WHERE code = 'sau-benh-hai'",
    )
    .await;
    let entry = page(&e, &format!("/admin/alert-kinds/{id}"), Some(&admin)).await;
    assert!(entry.contains("<code>sau-benh-hai</code>"));

    // An alert of it: the home page shows the kind's icon.
    let today = (Utc::now() + Duration::hours(7)).date_naive();
    let day = |d: i64| (today + Duration::days(d)).format("%Y-%m-%d").to_string();
    let res = send(
        &e.router,
        post_form(
            "/admin/alerts",
            &[
                ("kind", "sau-benh-hai"),
                ("sector", ""),
                ("province_code", ""),
                ("title", "Rầy nâu bùng phát"),
                ("body", "Thăm đồng mỗi ngày."),
                ("starts_on", &day(0)),
                ("ends_on", &day(3)),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let home = page(&e, "/", None).await;
    assert!(home.contains("Rầy nâu bùng phát") && home.contains("fa-bug"));
    let list = page(&e, "/admin/alerts", Some(&admin)).await;
    assert!(list.contains("Sâu bệnh hại"), "the kind's name in the list");

    // A kind that is not one of them is refused.
    let res = send(
        &e.router,
        post_form(
            "/admin/alerts",
            &[
                ("kind", "lu-lut"),
                ("title", "Lũ"),
                ("body", "."),
                ("starts_on", &day(0)),
                ("ends_on", &day(1)),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res)
        .await
        .contains("Vui lòng chọn loại cảnh báo."));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_admin_lists_say_each_rows_type_and_sector() {
    let e = env_with_origin().await;
    let actor = first_admin(&e.state).await;
    e.state.usecases.setup.load_demo(&actor).await.unwrap();
    let admin = sign_in(&e, "quantri", "matkhau123").await;

    let feed = scalar(
        &e,
        "SELECT supply_type_id::text AS v FROM products WHERE name = 'Cám heo thịt 25 kg'",
    )
    .await;
    let supplies = page(&e, "/admin/supplies", Some(&admin)).await;
    assert!(
        supplies.contains("<th>Phân loại</th>"),
        "type and sector share a column"
    );
    assert!(!supplies.contains("<th>Ngành</th>"));
    // What a supply is for is its own column, not part of its classification.
    assert!(supplies.contains("<th>Dùng cho</th>"));
    assert!(supplies.contains(&format!(r#"href="/admin/supplies?category_id={feed}""#)));
    // Pig feed is for pigs, so livestock, though feed serves every sector.
    let row = supplies
        .split("Cám heo thịt 25 kg")
        .nth(1)
        .and_then(|rest| rest.split("</tr>").next())
        .expect("the feed row");
    assert!(row.contains("Chăn nuôi"), "{row}");
    assert!(
        row.contains("Loại:") && row.contains("Ngành:") && !row.contains("Dùng cho:"),
        "{row}"
    );
    // Each label leads somewhere: the type and the commodity narrow the
    // list, the sector opens its page.
    assert!(
        row.contains(r#"href="/admin/sectors/"#)
            && row.contains(r#"href="/admin/supplies?commodity_id="#),
        "{row}"
    );

    let services = page(&e, "/admin/services", Some(&admin)).await;
    assert!(
        services.contains("<th>Phân loại</th>")
            && services.contains("Mọi ngành")
            && services.contains(r#"href="/admin/categories/"#)
    );
    e.cleanup().await;
}
