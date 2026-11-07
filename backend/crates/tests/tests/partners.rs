//! Partners: contracts by area and category, the revenue figures, the
//! partner's own report (only what it pays for), and its sponsored providers
//! shown first, labelled, on the near-me page of their area. Database tests
//! need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{
    body_text, categories_of, env_with_origin, get_page, install_admin, json_body, post_form,
    register_provider, send, sign_in, Env, TEST_ORIGIN,
};
use sea_orm::{ConnectionTrait, Statement};

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

/// A provider with one service in the first service category, placed at
/// (lng, lat); returns its business id and the category id.
async fn provider_at(e: &Env, username: &str, phone: &str, lng: f64, lat: f64) -> (i32, i32) {
    let token = register_provider(&e.state, username, phone).await;
    let category = categories_of(e, "service").await[0].0;
    let req = Request::post("/api/provider/services")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, TEST_ORIGIN)
        .body(Body::from(format!(
            r#"{{"category_id":{category},"name":"Phun drone {username}"}}"#
        )))
        .unwrap();
    assert_eq!(send(&e.router, req).await.status(), StatusCode::CREATED);
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
             ST_GeographyFromText('SRID=4326;POINT({lng} {lat})') WHERE id = {business}"
        ),
    )
    .await;
    (business, category)
}

async fn location(e: &Env, label: &str) -> String {
    scalar(
        e,
        &format!("SELECT id::text AS v FROM locations WHERE label = '{label}'"),
    )
    .await
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_partner_pays_for_an_area_reads_its_report_and_sponsors_a_shop() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    // Two shops by Cai Lậy (the second is sponsored), one in Long Xuyên.
    let (_plain, category) = provider_at(&e, "thuong", "0901111111", 106.090, 10.420).await;
    let (sponsored, _) = provider_at(&e, "taitro", "0902222222", 106.085, 10.415).await;
    let (far, _) = provider_at(&e, "longxuyen", "0903333333", 105.446, 10.374).await;
    let cai_lay = location(&e, "Đồng Tháp — Cai Lậy").await;

    // A partner: the kind is checked.
    let res = send(
        &e.router,
        post_form(
            "/admin/partners",
            &[("name", "Dai ly Drone"), ("kind", "bank")],
            Some(&admin),
        ),
    )
    .await;
    assert!(res.status().is_client_error());
    let res = send(
        &e.router,
        post_form(
            "/admin/partners",
            &[
                ("name", "Dai ly Drone"),
                ("kind", "dealer"),
                ("phone", "0912000111"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let page = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    let id = scalar(&e, "SELECT id::text AS v FROM partners").await;

    // A contract: an end before the start and no category are refused at once.
    let contracts = format!("{page}/contracts");
    let res = send(
        &e.router,
        post_form(
            &contracts,
            &[
                ("location_id", &cai_lay),
                ("starts_on", "2026-12-01"),
                ("ends_on", "2026-11-01"),
                ("monthly_fee", "2.000.000"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert!(res.status().is_client_error());
    let html = body_text(res).await;
    assert!(html.contains("Ngày kết thúc phải sau hoặc bằng ngày bắt đầu."));
    assert!(html.contains("Chọn ít nhất một danh mục."));
    let category_box = format!("cat.{category}");
    let res = send(
        &e.router,
        post_form(
            &contracts,
            &[
                ("location_id", &cai_lay),
                ("starts_on", "2020-01-01"),
                ("ends_on", "2099-12-31"),
                ("monthly_fee", "2.000.000"),
                (&category_box, "on"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let contract = scalar(&e, "SELECT id::text AS v FROM partner_contracts").await;

    // Sponsoring: only a shop of the contract's area.
    let sponsor = format!("{page}/contracts/{contract}/sponsored");
    let res = send(
        &e.router,
        post_form(&sponsor, &[("business_id", &far.to_string())], Some(&admin)),
    )
    .await;
    assert!(res.status().is_client_error());
    let res = send(
        &e.router,
        post_form(
            &sponsor,
            &[("business_id", &sponsored.to_string())],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // The revenue: one paying partner, 2 000 000 a month in Cai Lậy.
    let list = body_text(send(&e.router, get_page("/admin/partners", Some(&admin))).await).await;
    assert!(list.contains("Dai ly Drone"));
    assert!(list.contains("Đồng Tháp — Cai Lậy"));
    assert!(list.contains("2.000.000"));
    let dashboard =
        body_text(send(&e.router, get_page("/admin/dashboard", Some(&admin))).await).await;
    assert!(dashboard.contains("Đối tác đang trả phí"));

    // The near-me page of Cai Lậy: the sponsored shop first, labelled.
    let near = body_text(
        send(
            &e.router,
            get_page(&format!("/explore?location_id={cai_lay}"), None),
        )
        .await,
    )
    .await;
    let tag = near.find("Tài trợ").expect("labelled");
    let first = near.find("Phun drone taitro").expect("sponsored shown");
    let other = near
        .find("Phun drone thuong")
        .expect("the others still shown");
    assert!(tag < first && first < other, "sponsored first");
    assert_eq!(near.matches("Phun drone taitro").count(), 1, "never twice");

    // Renewed at a new price: edited in place, the area stays.
    let res = send(
        &e.router,
        post_form(
            &format!("{page}/contracts/{contract}"),
            &[
                ("starts_on", "2020-01-01"),
                ("ends_on", "2099-12-31"),
                ("monthly_fee", "3.000.000"),
                (&category_box, "on"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let list = body_text(send(&e.router, get_page("/admin/partners", Some(&admin))).await).await;
    assert!(list.contains("3.000.000"), "the new price counts");
    let detail = body_text(send(&e.router, get_page(&page, Some(&admin))).await).await;
    assert!(
        detail.contains(&format!(r##"href="#bo-tai-tro-{contract}-{sponsored}""##)),
        "stopping a sponsorship asks first"
    );

    // The partner account reads its own report, nothing else.
    let res = send(
        &e.router,
        post_form(
            &format!("{page}/account"),
            &[
                ("username", "dailydrone"),
                ("email", "daily@example.vn"),
                ("password", "matkhau123"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let partner = sign_in(&e, "dailydrone", "matkhau123").await;
    let res = send(&e.router, get_page("/login", Some(&partner))).await;
    assert_eq!(res.headers()[header::LOCATION], "/partner", "its own area");
    let own = body_text(send(&e.router, get_page("/partner", Some(&partner))).await).await;
    assert!(own.contains("Dai ly Drone"));
    assert!(!own.contains("bottom-nav"), "its own frame: no farmer tabs");
    assert!(own.contains("Đồng Tháp — Cai Lậy"));
    assert!(
        !own.contains("An Giang — Long Xuyên"),
        "only the areas it pays for"
    );
    let res = send(&e.router, get_page("/admin/demand", Some(&partner))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = send(&e.router, get_page("/partner/report.csv", Some(&partner))).await;
    assert_eq!(
        res.headers()[header::CONTENT_TYPE],
        "text/csv; charset=utf-8"
    );

    // Stopping the sponsorship, with a reason, is audited.
    let res = send(
        &e.router,
        post_form(
            &format!("{page}/contracts/{contract}/sponsored/{sponsored}/delete"),
            &[("reason", "het han")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // Turned off: no report, no sponsored place.
    let res = send(
        &e.router,
        post_form(
            &format!("{page}/status"),
            &[("is_active", "false"), ("reason", "het hop dong")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let res = send(&e.router, get_page("/partner", Some(&partner))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let near = body_text(
        send(
            &e.router,
            get_page(&format!("/explore?location_id={cai_lay}"), None),
        )
        .await,
    )
    .await;
    assert!(!near.contains("badge text-bg-warning"));
    assert_eq!(
        scalar(
            &e,
            &format!(
                "SELECT string_agg(action, ',' ORDER BY id) AS v FROM audit_logs \
                 WHERE entity_type IN ('partner', 'contract') AND entity_id IN ({id}, {contract})"
            ),
        )
        .await,
        "create_partner,create_contract,update_contract,update_contract,assign_partner_account,update_contract,hide_partner"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_json_api_manages_partners_and_reports_their_revenue() {
    let e = env_with_origin().await;
    install_admin(&e).await;
    let token = e
        .state
        .usecases
        .auth
        .login("quantri".into(), "matkhau123".into())
        .await
        .unwrap()
        .token;
    let json = |method: &str, path: &str, body: &str| {
        Request::builder()
            .method(method)
            .uri(path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, TEST_ORIGIN)
            .body(Body::from(body.to_owned()))
            .unwrap()
    };
    let res = send(
        &e.router,
        json(
            "POST",
            "/api/admin/partners",
            r#"{"name":"HTX Cai Lay","kind":"cooperative"}"#,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let id = json_body(res).await["partner"]["id"].as_i64().unwrap();
    let area = location(&e, "Đồng Tháp — Cai Lậy").await;
    let category = categories_of(&e, "service").await[0].0;
    let res = send(
        &e.router,
        json(
            "POST",
            &format!("/api/admin/partners/{id}/contracts"),
            &format!(
                r#"{{"location_id":{area},"starts_on":"2020-01-01","ends_on":"2099-12-31","monthly_fee":500000,"category_ids":[{category}]}}"#
            ),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let detail = json_body(res).await;
    assert_eq!(detail["contracts"][0]["in_force"], true);
    assert_eq!(detail["partner"]["monthly_revenue"], 500_000);
    let kpis = json_body(send(&e.router, json("GET", "/api/admin/partners/kpis", "")).await).await;
    assert_eq!(kpis["paying_partners"], 1);
    assert_eq!(kpis["areas"][0]["monthly_revenue"], 500_000);
    e.cleanup().await;
}
