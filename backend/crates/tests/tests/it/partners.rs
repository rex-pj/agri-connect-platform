//! Partners: contracts by area and category, the revenue figures, the
//! partner's own report (only what it pays for), and its sponsored providers
//! shown first, labelled, on the near-me page of their area. Database tests
//! need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{
    body_text, categories_of, env_with_origin, get_page, install_admin, json_body,
    load_admin_units, place_business, post_form, register_provider, send, sign_in, Env,
    TEST_ORIGIN,
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
    place_business(e, business, lat, lng).await;
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
    load_admin_units(&e).await;
    let admin = install_admin(&e).await;
    // Two shops in Xã Bình Phú by Cai Lậy (the second is sponsored), one in
    // Phường Long Xuyên.
    let (plain, category) = provider_at(&e, "thuong", "0901111111", 106.090, 10.420).await;
    let (sponsored, _) = provider_at(&e, "taitro", "0902222222", 106.085, 10.415).await;
    let (far, _) = provider_at(&e, "longxuyen", "0903333333", 105.446, 10.374).await;
    let cai_lay = location(&e, "Cai Lậy, Đồng Tháp").await;
    // The contract covers the commune the preset's point lies in.
    let binh_phu = "w28471";

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
    // A new partner lands on its contracts: the first one is next.
    let location = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    let (page, tab) = location.split_once('?').unwrap();
    assert_eq!(tab, "tab=contracts");
    let page = page.to_owned();
    let id = scalar(&e, "SELECT id::text AS v FROM partners").await;

    // A contract: an end before the start and no category are refused at once.
    let contracts = format!("{page}/contracts");
    let res = send(
        &e.router,
        post_form(
            &contracts,
            &[
                ("area", binh_phu),
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
    assert!(html.contains("Chọn ít nhất một loại dịch vụ hoặc vật tư."));
    let category_box = format!("cat.{category}");
    let res = send(
        &e.router,
        post_form(
            &contracts,
            &[
                ("area", binh_phu),
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

    // The revenue: one paying partner, 2 000 000 a month in Xã Bình Phú.
    let list = body_text(send(&e.router, get_page("/admin/partners", Some(&admin))).await).await;
    assert!(list.contains("Dai ly Drone"));
    assert!(list.contains("Xã Bình Phú, Đồng Tháp"));
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
    // The partner's contracts are one line each, leading to the contract's
    // own page, where its shops are sponsored.
    let tab = format!("{page}?tab=contracts");
    let detail = body_text(send(&e.router, get_page(&tab, Some(&admin))).await).await;
    let contract_page = format!("{page}/contracts/{contract}");
    assert!(detail.contains(&format!(r#"href="{contract_page}""#)));
    assert!(
        !detail.contains("bo-tai-tro-"),
        "no sponsor forms on the partner"
    );
    let html = body_text(send(&e.router, get_page(&contract_page, Some(&admin))).await).await;
    assert!(
        html.contains(r##"href="#bo-tai-tro-"##)
            && html.contains(&format!("bo-tai-tro-{sponsored}")),
        "stopping a sponsorship asks first"
    );
    // The search lists the other shops of its area, never one outside it
    // nor one sponsored already.
    let other_shop = |h: &str| h.contains(&format!(r#"name="business_id" value="{plain}""#));
    assert!(other_shop(&html), "the area's other shop can be added");
    assert!(!html.contains(&format!(r#"name="business_id" value="{far}""#)));
    assert!(!html.contains(&format!(r#"name="business_id" value="{sponsored}""#)));
    let found = format!("{contract_page}?q=thuong");
    let html = body_text(send(&e.router, get_page(&found, Some(&admin))).await).await;
    assert!(other_shop(&html), "found by name");
    let none = format!("{contract_page}?q=khongcoai");
    let html = body_text(send(&e.router, get_page(&none, Some(&admin))).await).await;
    assert!(
        !html.contains(r#"name="business_id""#),
        "nothing by that name"
    );
    // Its terms are the other part.
    let terms = format!("{contract_page}?tab=terms");
    let html = body_text(send(&e.router, get_page(&terms, Some(&admin))).await).await;
    assert!(html.contains(r#"name="monthly_fee""#) && html.contains("3000000"));
    // Its contracts' changes are its own history, each named by its area.
    let history = format!("{page}?tab=history");
    let html = body_text(send(&e.router, get_page(&history, Some(&admin))).await).await;
    assert!(html.contains("Thêm hợp đồng"));
    assert!(html.contains("Sửa hợp đồng"));
    assert!(html.contains("Tài trợ nhà cung cấp"));

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
    assert!(own.contains("Xã Bình Phú, Đồng Tháp"));
    assert!(
        own.contains(r#"href="/account""#),
        "the account menu leads to the password change"
    );
    let profile = body_text(send(&e.router, get_page("/account", Some(&partner))).await).await;
    assert!(profile.contains(r#"id="security""#));
    assert!(
        !profile.contains("bottom-nav"),
        "the profile stays in the partner frame"
    );
    assert!(
        profile.contains(r#"href="/partner""#),
        "the way back to the report"
    );
    assert!(!own.contains("Long Xuyên"), "only the areas it pays for");
    let res = send(&e.router, get_page("/admin/demand", Some(&partner))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = send(&e.router, get_page("/partner/report.csv", Some(&partner))).await;
    assert_eq!(
        res.headers()[header::CONTENT_TYPE],
        "text/csv; charset=utf-8"
    );

    // The sponsored shop moves out of the contract's commune (its ward now
    // Long Xuyên's; its point kept, so it is still near Cai Lậy): it is no
    // longer moved up there, and the contract page says why.
    let move_to = |from: i32, to: i32| {
        format!(
            "UPDATE businesses SET ward_code = (SELECT ward_code FROM businesses WHERE id = {from}) \
             WHERE id = {to}"
        )
    };
    let home = scalar(
        &e,
        &format!("SELECT ward_code AS v FROM businesses WHERE id = {sponsored}"),
    )
    .await;
    e.t.db
        .execute_unprepared(&move_to(far, sponsored))
        .await
        .unwrap();
    let near_page = || async {
        body_text(
            send(
                &e.router,
                get_page(&format!("/explore?location_id={cai_lay}"), None),
            )
            .await,
        )
        .await
    };
    let near = near_page().await;
    assert!(
        near.contains("Phun drone taitro"),
        "still listed near Cai Lậy"
    );
    assert!(
        !near.contains(r#"aria-label="Tài trợ""#),
        "but not as sponsored"
    );
    let html = body_text(send(&e.router, get_page(&contract_page, Some(&admin))).await).await;
    assert!(html.contains("Đã dời khỏi khu vực của hợp đồng"));
    // Back home: sponsored again.
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE businesses SET ward_code = '{home}' WHERE id = {sponsored}"
        ))
        .await
        .unwrap();
    assert!(near_page().await.contains(r#"aria-label="Tài trợ""#));

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
    assert!(!near.contains(r#"aria-label="Tài trợ""#));
    assert_eq!(
        scalar(
            &e,
            &format!(
                "SELECT string_agg(action, ',' ORDER BY id) AS v FROM audit_logs \
                 WHERE entity_type = 'partner' AND entity_id = {id}"
            ),
        )
        .await,
        // A contract's changes are audited on its partner, each by what it was.
        "create_partner,add_partner_contract,sponsor_partner_shop,update_partner_contract,assign_partner_account,unsponsor_partner_shop,hide_partner"
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
        .login(
            "quantri".into(),
            "matkhau123".into(),
            integration_tests::CLIENT,
        )
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
    load_admin_units(&e).await;
    let category = categories_of(&e, "service").await[0].0;
    // A preset id is no area any more; a whole province is.
    let res = send(
        &e.router,
        json(
            "POST",
            &format!("/api/admin/partners/{id}/contracts"),
            &format!(
                r#"{{"area":"12","starts_on":"2020-01-01","ends_on":"2099-12-31","monthly_fee":500000,"category_ids":[{category}]}}"#
            ),
        ),
    )
    .await;
    assert!(res.status().is_client_error());
    assert_eq!(
        json_body(res).await["field_errors"]["area"][0],
        "Không tìm thấy khu vực."
    );
    let res = send(
        &e.router,
        json(
            "POST",
            &format!("/api/admin/partners/{id}/contracts"),
            &format!(
                r#"{{"area":"p82","starts_on":"2020-01-01","ends_on":"2099-12-31","monthly_fee":500000,"category_ids":[{category}]}}"#
            ),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let detail = json_body(res).await;
    assert_eq!(detail["contracts"][0]["in_force"], true);
    assert_eq!(detail["contracts"][0]["area"], "p82");
    assert_eq!(detail["contracts"][0]["area_label"], "Đồng Tháp");
    assert_eq!(detail["partner"]["monthly_revenue"], 500_000);
    let kpis = json_body(send(&e.router, json("GET", "/api/admin/partners/kpis", "")).await).await;
    assert_eq!(kpis["paying_partners"], 1);
    assert_eq!(kpis["provinces"][0]["monthly_revenue"], 500_000);
    assert_eq!(kpis["provinces"][0]["label"], "Đồng Tháp");
    assert_eq!(kpis["provinces"][0]["areas"][0]["area"], "p82");
    e.cleanup().await;
}
