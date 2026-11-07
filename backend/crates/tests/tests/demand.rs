//! /admin/demand: leads by category × area × week, small figures hidden,
//! demo data and spam left out, the CSV and JSON of the same figures.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{
    body_text, categories_of, env_with_origin, get_page, install_admin, json_body,
    register_provider, send, sign_in, Env, TEST_ORIGIN,
};
use sea_orm::{ConnectionTrait, Statement};

/// Wednesday 10:00 of last week, Vietnam time: inside the report's last week.
const LAST_WEEK: &str = "((date_trunc('week', now() AT TIME ZONE 'Asia/Ho_Chi_Minh') \
     - interval '5 days' + interval '10 hours') AT TIME ZONE 'Asia/Ho_Chi_Minh')";

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

async fn exec(e: &Env, sql: &str) {
    e.t.db.execute_unprepared(sql).await.unwrap();
}

/// The provider `username`'s business, placed by Cai Lậy, with one service
/// in the first service category; returns (business id, service id, its
/// category name).
async fn provider_at_cai_lay(e: &Env, username: &str, phone: &str) -> (i32, i32, String) {
    let token = register_provider(&e.state, username, phone).await;
    let (category, _) = categories_of(e, "service").await.remove(0);
    let name = scalar(
        e,
        &format!("SELECT name AS v FROM categories WHERE id = {category}"),
    )
    .await;
    let req = Request::post("/api/provider/services")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, TEST_ORIGIN)
        .body(Body::from(format!(r#"{{"category_id":{category}}}"#)))
        .unwrap();
    let service = json_body(send(&e.router, req).await).await["id"]
        .as_i64()
        .unwrap() as i32;
    let business: i32 = scalar(
        e,
        &format!("SELECT id::text AS v FROM businesses WHERE name = 'Vườn {username}'"),
    )
    .await
    .parse()
    .unwrap();
    exec(
        e,
        &format!(
            "UPDATE businesses SET location = \
             ST_GeographyFromText('SRID=4326;POINT(106.085 10.415)') WHERE id = {business}"
        ),
    )
    .await;
    (business, service, name)
}

async fn taps(e: &Env, business: i32, service: i32, channel: &str, n: usize) {
    for _ in 0..n {
        exec(
            e,
            &format!(
                "INSERT INTO call_clicks (business_id, service_id, channel, created_on) \
                 VALUES ({business}, {service}, '{channel}', {LAST_WEEK})"
            ),
        )
        .await;
    }
}

async fn inquiry(e: &Env, business: i32, service: i32, phone: &str, status: &str) {
    exec(
        e,
        &format!(
            "INSERT INTO inquiries (business_id, service_id, topic, requester_name, \
             requester_phone, status, created_on) \
             VALUES ({business}, {service}, 't', 'n', '{phone}', '{status}', {LAST_WEEK})"
        ),
    )
    .await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_report_counts_real_leads_by_area_and_hides_small_figures() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let (business, service, category) = provider_at_cai_lay(&e, "nongdan", "0901234567").await;
    // 6 calls, 2 Zalo, 2 requests from one phone, 1 spam: 10 leads.
    taps(&e, business, service, "call", 6).await;
    taps(&e, business, service, "zalo", 2).await;
    inquiry(&e, business, service, "0912345678", "new").await;
    inquiry(&e, business, service, "0912345678", "contacted").await;
    inquiry(&e, business, service, "0999999999", "spam").await;
    // A demo shop's taps never count.
    let (demo, demo_service, _) = provider_at_cai_lay(&e, "mau", "0907654321").await;
    exec(
        &e,
        &format!("UPDATE businesses SET is_demo = TRUE WHERE id = {demo}"),
    )
    .await;
    taps(&e, demo, demo_service, "call", 30).await;

    let res = send(&e.router, get_page("/admin/demand", Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::OK);
    let page = body_text(res).await;
    assert!(page.contains(&category));
    assert!(page.contains("Đồng Tháp — Cai Lậy"), "the area of the shop");
    assert!(page.contains("&lt; 5"), "2 Zalo taps are hidden");

    // JSON: the same figures, a hidden one is null.
    let token = e
        .state
        .usecases
        .auth
        .login("quantri".into(), "matkhau123".into())
        .await
        .unwrap()
        .token;
    let req = Request::get("/api/admin/reports/demand")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let json = json_body(send(&e.router, req).await).await;
    let row = &json["rows"][0];
    assert_eq!(row["calls"]["value"], 6);
    assert_eq!(row["zalo"]["value"], serde_json::Value::Null);
    assert_eq!(
        row["inquiries"]["value"],
        serde_json::Value::Null,
        "2 requests: hidden"
    );
    assert_eq!(row["weeks"][7]["value"], 10, "no spam, no demo");
    assert_eq!(
        row["total"]["value"], 10,
        "two hidden parts give neither away"
    );
    let cai_lay = json["areas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["label"] == "Đồng Tháp — Cai Lậy")
        .unwrap()
        .clone();
    assert_eq!(cai_lay["total"]["value"], 10);
    assert_eq!(cai_lay["steady"], false);

    // Narrowed to another area: no rows.
    let other = scalar(
        &e,
        "SELECT id::text AS v FROM locations WHERE label = 'An Giang — Long Xuyên'",
    )
    .await;
    let page = body_text(
        send(
            &e.router,
            get_page(&format!("/admin/demand?area={other}"), Some(&admin)),
        )
        .await,
    )
    .await;
    assert!(page.contains("Chưa có liên hệ nào trong các tuần này."));

    // CSV: a header line, the category, the hidden figure as "< 5".
    let res = send(&e.router, get_page("/admin/demand.csv", Some(&admin))).await;
    assert_eq!(
        res.headers()[header::CONTENT_TYPE],
        "text/csv; charset=utf-8"
    );
    let csv = body_text(res).await;
    assert!(csv.starts_with('\u{feff}'));
    assert!(csv.contains(&category));
    assert!(csv.contains("< 5"));

    let dashboard =
        body_text(send(&e.router, get_page("/admin/dashboard", Some(&admin))).await).await;
    assert!(dashboard.contains("Khu vực đều đặn (8 tuần)"));
    assert!(dashboard.contains(r#"href="/admin/demand""#));

    // A provider cannot read it.
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    let res = send(&e.router, get_page("/admin/demand", Some(&provider))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}
