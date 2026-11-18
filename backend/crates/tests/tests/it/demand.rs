//! /admin/demand: leads by category × area × week, small figures hidden,
//! demo data and spam left out, the CSV and JSON of the same figures.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{
    body_text, categories_of, env_with_origin, get_page, install_admin, json_body,
    load_admin_units, place_business, register_provider, send, send_from, sign_in, Env,
    TEST_ORIGIN,
};
use sea_orm::{ConnectionTrait, Statement};

/// Wednesday 10:00 of last week, Vietnam time: inside the report's last week.
const LAST_WEEK: &str = "((date_trunc('week', now() AT TIME ZONE 'Asia/Ho_Chi_Minh') \
     - interval '5 days' + interval '10 hours') AT TIME ZONE 'Asia/Ho_Chi_Minh')";

/// The commune the shops of these tests lie in (by Cai Lậy, Đồng Tháp), and
/// what the report calls it.
const BINH_PHU: &str = "28471";
const BINH_PHU_LABEL: &str = "Đồng Tháp — Xã Bình Phú";

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
        &format!("SELECT name AS v FROM service_types WHERE id = {category}"),
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
    place_business(e, business, 10.415, 106.085).await;
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
    load_admin_units(&e).await;
    let admin = install_admin(&e).await;
    let (business, service, category) = provider_at_cai_lay(&e, "nongdan", "0901234567").await;
    // 6 calls, 2 Zalo, 2 requests from one phone, 1 spam: 10 leads.
    taps(&e, business, service, "call", 6).await;
    taps(&e, business, service, "zalo", 2).await;
    inquiry(&e, business, service, "0912345678", "new").await;
    inquiry(&e, business, service, "0912345678", "contacted").await;
    inquiry(&e, business, service, "0999999999", "spam").await;

    let res = send(&e.router, get_page("/admin/demand", Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::OK);
    let page = body_text(res).await;
    assert!(page.contains(&category));
    assert!(page.contains("Xã Bình Phú"), "the commune of the shop");
    assert!(
        page.contains(r#"<option value="p82">Đồng Tháp</option>"#),
        "its province"
    );
    assert!(page.contains("&lt; 5"), "2 Zalo taps are hidden");

    // JSON: the same figures, a hidden one is null.
    let token = e
        .state
        .usecases
        .auth
        .login(
            "quantri".into(),
            "matkhau123".into(),
            integration_tests::CLIENT,
        )
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
    assert_eq!(row["weeks"][7]["value"], 10, "no spam");
    assert_eq!(json["with_demo"], false, "no demo data loaded");
    assert_eq!(
        row["total"]["value"], 10,
        "two hidden parts give neither away"
    );
    let cai_lay = json["areas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["label"] == BINH_PHU_LABEL)
        .unwrap()
        .clone();
    assert_eq!(cai_lay["value"], format!("w{BINH_PHU}"));
    assert_eq!(cai_lay["total"]["value"], 10);
    assert_eq!(cai_lay["weeks"][7]["value"], 10, "its leads by week");
    assert_eq!(cai_lay["weeks"][0]["value"], 0);
    assert_eq!(cai_lay["weeks_at_bar"], 0);
    assert_eq!(cai_lay["steady"], false);
    // The page shows how near it is to steady: its average a week against
    // the goal (10 leads over 8 weeks: 1), its weeks as squares, each named
    // by its week and figure, none reached; each area leads to its report.
    assert!(page.contains("admin-table area-progress"));
    assert!(page.contains("1/tuần"));
    assert!(page.contains(r#": 10"></span>"#));
    assert!(!page.contains("area-progress__week--met"));
    assert!(page.contains("0/8 tuần"));
    assert!(page.contains(&format!(r#"href="/admin/demand?area=w{BINH_PHU}""#)));
    // The category × week table is shaded: the one week with leads is the
    // darkest, the legend under it.
    assert!(page.contains("demand-heat demand-heat--4"));
    assert!(
        !page.contains("demand-heat demand-heat--0"),
        "a zero is not shaded"
    );
    assert!(page.contains("Màu càng đậm, tuần đó càng nhiều liên hệ."));
    assert_eq!(json["areas"][0]["value"], "p82", "the province first");
    assert_eq!(json["areas"][0]["total"]["value"], 10);

    // Narrowed to the province: its commune's leads; to another: no rows.
    let page =
        body_text(send(&e.router, get_page("/admin/demand?area=p82", Some(&admin))).await).await;
    assert!(page.contains(&category));
    let page =
        body_text(send(&e.router, get_page("/admin/demand?area=p91", Some(&admin))).await).await;
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
    assert!(dashboard.contains("Khu vực đều đặn"));
    assert!(dashboard.contains("8 tuần liền, mỗi tuần từ 20 liên hệ"));
    assert!(dashboard.contains(r#"href="/admin/demand""#));
    assert!(dashboard.contains("Khu vực gần đều đặn"));
    assert!(
        dashboard.contains(&format!(r#"href="/admin/demand?area=w{BINH_PHU}""#)),
        "each area leads to its own report"
    );
    assert!(dashboard.contains("admin-table area-progress"));
    // As on the report: the province a top row, its commune under it by its
    // own name, the last of its group, never the full "province — ward".
    let top = dashboard
        .find(r#"class="area-progress__top""#)
        .expect("the province");
    let ward = dashboard
        .find(r#"class="area-progress__ward area-progress__ward--last""#)
        .expect("its commune");
    assert!(top < ward, "the commune follows its province");
    assert!(!dashboard.contains("Đồng Tháp — "));
    // The cards say how they moved: one provider listed this week, 8 taps
    // against none in the 30 days before.
    // The figure (an arrow and the sign) apart from its words.
    assert!(dashboard.contains("stat-card__delta--up"));
    assert!(dashboard.contains("</i> +1</span>") && dashboard.contains("mới trong 7 ngày"));
    assert!(dashboard.contains("</i> +8</span>") && dashboard.contains("so với 30 ngày trước"));
    // The weekly leads as columns, each labelled with its figure.
    assert!(dashboard.contains(r#"<svg class="week-columns""#));
    assert!(dashboard.contains(r#"week-columns__bar--leads"#));
    assert!(dashboard.contains(r#"text-anchor="middle">10</text>"#));
    assert!(
        dashboard.contains("Xem bảng số liệu"),
        "the table folded under"
    );

    // A provider cannot read it.
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    let res = send(&e.router, get_page("/admin/demand", Some(&provider))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}

/// Waits for the background search records to land.
async fn searches_stored(e: &Env, n: i64) {
    for _ in 0..100 {
        let stored: i64 = scalar(e, "SELECT count(*)::text AS v FROM search_events")
            .await
            .parse()
            .unwrap();
        if stored >= n {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("searches were not recorded");
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn searches_are_recorded_without_who_and_unmet_ones_are_listed() {
    let e = env_with_origin().await;
    load_admin_units(&e).await;
    let admin = install_admin(&e).await;
    // Five farmers search the same words; the first one reloads the page,
    // which is the same search.
    for n in 0..5_u8 {
        let res = send_from(
            &e.router,
            get_page("/search?q=+May+Xoi+Lua+", None),
            [10, 0, 0, n],
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
    }
    send_from(
        &e.router,
        get_page("/search?q=may+xoi+lua", None),
        [10, 0, 0, 0],
    )
    .await;
    // A tab switch is the same search, and so is narrowing it to a sector; a
    // page without words is none.
    send(
        &e.router,
        get_page("/search?q=may+xoi+lua&tab=products", None),
    )
    .await;
    send(
        &e.router,
        get_page("/search?q=may+xoi+lua&sector=crop", None),
    )
    .await;
    send(&e.router, get_page("/search?q=", None)).await;
    let cai_lay = scalar(
        &e,
        "SELECT id::text AS v FROM locations WHERE label = 'Đồng Tháp — Cai Lậy'",
    )
    .await;
    send(
        &e.router,
        get_page(&format!("/explore?location_id={cai_lay}"), None),
    )
    .await;
    // A device position is filed under the commune it lies in.
    send(
        &e.router,
        get_page("/explore?lat=10.4150&lng=106.0850", None),
    )
    .await;
    searches_stored(&e, 7).await;
    // A signed-in admin trying the search is not demand.
    send(&e.router, get_page("/search?q=thu+nghiem", Some(&admin))).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(
        scalar(&e, "SELECT count(*)::text AS v FROM search_events").await,
        "7",
        "no reload, no tab switch, no admin"
    );

    assert_eq!(
        scalar(
            &e,
            "SELECT string_agg(DISTINCT coalesce(query, '-'), ',') AS v FROM search_events"
        )
        .await,
        "-,may xoi lua",
        "trimmed and lowercased"
    );
    assert_eq!(
        scalar(
            &e,
            &format!(
                "SELECT count(*)::text AS v FROM search_events WHERE ward_code = '{BINH_PHU}'"
            )
        )
        .await,
        "2"
    );
    // Moved into the report window: last week.
    exec(
        &e,
        &format!("UPDATE search_events SET created_on = {LAST_WEEK}"),
    )
    .await;
    let page = body_text(send(&e.router, get_page("/admin/demand", Some(&admin))).await).await;
    assert!(page.contains("Bà con tìm mà không thấy"));
    assert!(page.contains("“may xoi lua”"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn taps_and_searches_older_than_any_report_become_weekly_totals() {
    let e = env_with_origin().await;
    let (business, service, _) = provider_at_cai_lay(&e, "thomay", "0911111111").await;
    // Last week's stay as they are; those of a year ago are past keeping:
    // three calls and a Zalo tap in one week, one call the week after.
    taps(&e, business, service, "call", 2).await;
    for (channel, back, n) in [
        ("call", "52 weeks", 3),
        ("zalo", "52 weeks", 1),
        ("call", "51 weeks", 1),
    ] {
        for _ in 0..n {
            exec(
                &e,
                &format!(
                    "INSERT INTO call_clicks (business_id, service_id, channel, created_on) \
                     VALUES ({business}, {service}, '{channel}', {LAST_WEEK} - interval '{back}')"
                ),
            )
            .await;
        }
    }
    exec(
        &e,
        &format!(
            "INSERT INTO search_events (query, results, created_on) VALUES \
             ('may cay', 0, {LAST_WEEK} - interval '52 weeks'), \
             ('may cay', 0, {LAST_WEEK} - interval '52 weeks'), \
             ('phun thuoc', 4, {LAST_WEEK} - interval '52 weeks'), \
             ('phun thuoc', 4, {LAST_WEEK})"
        ),
    )
    .await;

    let demand = &e.state.usecases.demand;
    let deleted = demand.roll_up_old_events(chrono::Utc::now()).await.unwrap();
    assert_eq!(deleted, 5 + 3, "the old taps and searches");
    assert_eq!(
        scalar(&e, "SELECT count(*)::text AS v FROM call_clicks").await,
        "2",
        "what a report still reads stays"
    );
    assert_eq!(
        scalar(&e, "SELECT count(*)::text AS v FROM search_events").await,
        "1"
    );
    // One row per week, business, type and channel, each week a Monday.
    assert_eq!(
        scalar(
            &e,
            "SELECT string_agg(channel || ':' || taps, ',' ORDER BY week_starts_on, channel) AS v \
             FROM call_click_weeks WHERE service_type_id IS NOT NULL \
               AND extract(isodow FROM week_starts_on) = 1"
        )
        .await,
        "call:3,zalo:1,call:1"
    );
    assert_eq!(
        scalar(
            &e,
            "SELECT searches || ' of which ' || unmet AS v FROM search_event_weeks"
        )
        .await,
        "3 of which 2",
        "the words are not kept"
    );
    // Nothing left to do: a second run changes nothing.
    assert_eq!(
        demand.roll_up_old_events(chrono::Utc::now()).await.unwrap(),
        0
    );
    assert_eq!(
        scalar(&e, "SELECT sum(taps)::text AS v FROM call_click_weeks").await,
        "5"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_business_keeps_the_ward_of_its_position() {
    let e = env_with_origin().await;
    let token = register_provider(&e.state, "nongdan", "0901234567").await;
    let ward = || async {
        scalar(
            &e,
            "SELECT coalesce(ward_code, '-') AS v FROM businesses WHERE name = 'Vườn nongdan'",
        )
        .await
    };
    let profile = |position: &str| {
        Request::put("/api/provider/profile")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, TEST_ORIGIN)
            .body(Body::from(format!(
                r#"{{"name":"Vườn nongdan","phone":"0901234567"{position}}}"#
            )))
            .unwrap()
    };
    // Placed before the wards are loaded: no ward yet; loading them gives
    // every business the ward of its position.
    let by_cai_lay = r#","lat":10.415,"lng":106.085"#;
    assert_eq!(
        send(&e.router, profile(by_cai_lay)).await.status(),
        StatusCode::OK
    );
    assert_eq!(ward().await, "-");
    load_admin_units(&e).await;
    assert_eq!(ward().await, BINH_PHU);

    // The profile edit writes the ward with the position, and clears both.
    assert_eq!(send(&e.router, profile("")).await.status(), StatusCode::OK);
    assert_eq!(ward().await, "-");
    assert_eq!(
        send(&e.router, profile(by_cai_lay)).await.status(),
        StatusCode::OK
    );
    assert_eq!(ward().await, BINH_PHU);
    e.cleanup().await;
}
