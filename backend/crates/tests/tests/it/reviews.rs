//! Reviews: only a farmer the provider answered through the platform may
//! review it, the review waits for an admin, the provider answers it, and the
//! JSON API follows the same rules. Database tests need TEST_DATABASE_URL and
//! `--include-ignored`.

use application::inputs::ContactInput;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{
    body_text, env_with_origin, get_page, install_admin, json_body, post_form, register_provider,
    send, sign_in, Env, TEST_ORIGIN,
};
use sea_orm::{ConnectionTrait, Statement};

const FARMER: &str = "0912345678";

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

async fn business_of(e: &Env, username: &str) -> i32 {
    scalar(
        e,
        &format!("SELECT id::text AS v FROM businesses WHERE name = 'Vườn {username}'"),
    )
    .await
    .parse()
    .unwrap()
}

/// A farmer's request to `business_id` from `FARMER`; returns its id.
async fn inquiry_to(e: &Env, business_id: i32) -> i32 {
    e.state
        .usecases
        .explore
        .contact_request(ContactInput {
            business_id,
            topic: "Phun thuốc 2 ha".into(),
            requester_name: "Chú Tư".into(),
            requester_phone: FARMER.into(),
            note: None,
            service_id: None,
            product_id: None,
        })
        .await
        .unwrap()
        .id
}

fn review_form(business_id: i32, rating: &str) -> Vec<(&'static str, String)> {
    vec![
        ("business_id", business_id.to_string()),
        ("reviewer_name", "Chú Tư".into()),
        ("reviewer_phone", FARMER.into()),
        ("rating", rating.into()),
        ("comment", "Phun kỹ, đúng hẹn".into()),
    ]
}

fn post(path: &str, pairs: &[(&'static str, String)], cookie: Option<&str>) -> Request<Body> {
    let pairs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    post_form(path, &pairs, cookie)
}

async fn public_page(e: &Env, business_id: i32) -> String {
    let res = send(
        &e.router,
        get_page(&format!("/providers/{business_id}"), None),
    )
    .await;
    let canonical = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    body_text(send(&e.router, get_page(&canonical, None)).await).await
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn only_an_answered_farmer_reviews_and_an_admin_approves_it() {
    let e = env_with_origin().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let business = business_of(&e, "nongdan").await;
    let page = public_page(&e, business).await;
    assert!(page.contains("Chưa có đánh giá nào."));
    assert!(
        page.contains(r##"href="#danh-gia""##),
        "the sheet is offered"
    );

    // No contact request yet: refused, the sheet comes back open as typed.
    let res = send(
        &e.router,
        post("/reviews", &review_form(business, "5"), None),
    )
    .await;
    assert!(res.status().is_client_error());
    let html = body_text(res).await;
    assert!(html.contains("Số này chưa gửi yêu cầu liên hệ nào"));
    assert!(html.contains(r#"value="5" checked"#));

    // A request the provider has not answered yet does not count either.
    let inquiry = inquiry_to(&e, business).await;
    let res = send(
        &e.router,
        post("/reviews", &review_form(business, "5"), None),
    )
    .await;
    assert!(res.status().is_client_error());

    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    let res = send(
        &e.router,
        post_form(
            &format!("/provider/inquiries/{inquiry}/status"),
            &[("status", "contacted")],
            Some(&provider),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    // Called back: the inbox offers the review link to send on Zalo.
    let inbox =
        body_text(send(&e.router, get_page("/provider/inquiries", Some(&provider))).await).await;
    assert!(inbox.contains("Gửi link đánh giá"));
    assert!(inbox.contains("#danh-gia-list\""));

    // Every box is checked at once.
    let res = send(
        &e.router,
        post("/reviews", &review_form(business, "9"), None),
    )
    .await;
    assert!(body_text(res)
        .await
        .contains("Vui lòng chọn từ 1 đến 5 sao."));

    let res = send(
        &e.router,
        post("/reviews", &review_form(business, "4"), None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        scalar(
            &e,
            "SELECT (inquiry_id::text || ' ' || is_visible::text || ' ' || \
             (reviewer_phone_hash <> '0912345678')::text) AS v FROM reviews",
        )
        .await,
        format!("{inquiry} false true"),
        "tied to the request, hidden, the phone only hashed"
    );
    assert!(
        !public_page(&e, business)
            .await
            .contains("Phun kỹ, đúng hẹn"),
        "not shown before approval"
    );
    let res = send(
        &e.router,
        post("/reviews", &review_form(business, "5"), None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res)
        .await
        .contains("Số này đã đánh giá nhà cung cấp này rồi."));

    // The admin's "show" is the approval.
    let admin = install_admin(&e).await;
    let queue = body_text(send(&e.router, get_page("/admin/reviews", Some(&admin))).await).await;
    assert!(queue.contains("Chờ duyệt"));
    let review = scalar(&e, "SELECT id::text AS v FROM reviews").await;
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/reviews/{review}/visibility"),
            &[("is_visible", "true")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let page = public_page(&e, business).await;
    assert!(page.contains("Phun kỹ, đúng hẹn"));
    assert!(page.contains("<strong>4</strong>"), "the average");
    assert!(page.contains("1 đánh giá"));
    // The rating follows the shop: its cards in the directory too.
    let directory = body_text(send(&e.router, get_page("/providers", None)).await).await;
    assert!(
        directory.contains("<strong>4</strong>"),
        "rated on its card"
    );
    assert!(
        !page.contains(FARMER),
        "the reviewer's phone is never shown"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_provider_answers_its_own_reviews_only() {
    let e = env_with_origin().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    register_provider(&e.state, "hangxom", "0907654321").await;
    let business = business_of(&e, "nongdan").await;
    let inquiry = inquiry_to(&e, business).await;
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE inquiries SET status = 'closed' WHERE id = {inquiry}"
        ))
        .await
        .unwrap();
    let res = send(
        &e.router,
        post("/reviews", &review_form(business, "5"), None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let review = scalar(&e, "SELECT id::text AS v FROM reviews").await;

    let owner = sign_in(&e, "nongdan", "matkhau123").await;
    let dashboard =
        body_text(send(&e.router, get_page("/provider/dashboard", Some(&owner))).await).await;
    assert!(dashboard.contains(r#"href="/provider/reviews""#));
    let mine = body_text(send(&e.router, get_page("/provider/reviews", Some(&owner))).await).await;
    assert!(mine.contains("Phun kỹ, đúng hẹn"));
    assert!(mine.contains("1 đánh giá · 1 chờ duyệt"));

    let reply = format!("/provider/reviews/{review}/reply");
    let res = send(
        &e.router,
        post_form(&reply, &[("reply", &"x".repeat(501))], Some(&owner)),
    )
    .await;
    assert!(res.status().is_client_error());
    assert!(body_text(res).await.contains("Tối đa 500 ký tự."));

    let res = send(
        &e.router,
        post_form(&reply, &[("reply", "Cảm ơn chú Tư")], Some(&owner)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    e.t.db
        .execute_unprepared("UPDATE reviews SET is_visible = TRUE")
        .await
        .unwrap();
    assert!(public_page(&e, business).await.contains("Cảm ơn chú Tư"));
    assert_eq!(
        scalar(
            &e,
            "SELECT count(*)::text AS v FROM audit_logs WHERE action = 'update_review'"
        )
        .await,
        "1"
    );

    // Someone else's review looks missing.
    let neighbour = sign_in(&e, "hangxom", "matkhau123").await;
    let res = send(
        &e.router,
        post_form(&reply, &[("reply", "spam")], Some(&neighbour)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // A blank answer takes it down.
    let res = send(
        &e.router,
        post_form(&reply, &[("reply", "  ")], Some(&owner)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        scalar(
            &e,
            "SELECT (reply IS NULL AND replied_at IS NULL)::text AS v FROM reviews"
        )
        .await,
        "true"
    );
    e.cleanup().await;
}

fn json(method: &str, path: &str, token: Option<&str>, body: &str) -> Request<Body> {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, TEST_ORIGIN);
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    req.body(Body::from(body.to_owned())).unwrap()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_json_api_creates_providers_and_carries_reviews() {
    let e = env_with_origin().await;
    install_admin(&e).await;
    let admin = e
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

    let res = send(
        &e.router,
        json(
            "POST",
            "/api/admin/providers",
            Some(&admin),
            r#"{"name":"Drone Ba Tu","phone":"12"}"#,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(res).await["field_errors"]["phone"][0],
        "Số điện thoại không hợp lệ (ví dụ: 0901234567)."
    );

    let res = send(
        &e.router,
        json(
            "POST",
            "/api/admin/providers",
            Some(&admin),
            r#"{"name":"Drone Ba Tu","phone":"0912000111","lat":10.4048,"lng":106.118}"#,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created = json_body(res).await;
    let id = created["id"].as_i64().unwrap();
    assert_eq!(created["lat"], 10.4048);
    assert_eq!(created["linked_username"], serde_json::Value::Null);

    let res = send(
        &e.router,
        json(
            "PUT",
            &format!("/api/admin/providers/{id}"),
            Some(&admin),
            r#"{"name":"Drone Ba Tu Cai Lay","phone":"0912000111"}"#,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let updated = json_body(res).await;
    assert_eq!(updated["name"], "Drone Ba Tu Cai Lay");
    assert_eq!(updated["lat"], serde_json::Value::Null);

    // A provider cannot create one.
    let provider = register_provider(&e.state, "nongdan", "0901234567").await;
    let res = send(
        &e.router,
        json(
            "POST",
            "/api/admin/providers",
            Some(&provider),
            r#"{"name":"X","phone":"0912000111"}"#,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Reviews over JSON: same rules, then the owner's list and answer.
    let business = business_of(&e, "nongdan").await;
    let body = format!(
        r#"{{"business_id":{business},"reviewer_name":"Chú Tư","reviewer_phone":"{FARMER}","rating":5}}"#
    );
    let res = send(&e.router, json("POST", "/api/explore/reviews", None, &body)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(res).await["field_errors"]["reviewer_phone"][0],
        "Số này chưa gửi yêu cầu liên hệ nào được nhà cung cấp liên hệ lại. Hãy gửi yêu cầu và chờ họ gọi trước."
    );
    let inquiry = inquiry_to(&e, business).await;
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE inquiries SET status = 'contacted' WHERE id = {inquiry}"
        ))
        .await
        .unwrap();
    let res = send(&e.router, json("POST", "/api/explore/reviews", None, &body)).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let sent = json_body(res).await;
    assert_eq!(sent["is_visible"], false);
    let review = sent["id"].as_i64().unwrap();

    let res = send(
        &e.router,
        get_page(&format!("/api/explore/providers/{business}/reviews"), None),
    )
    .await;
    let shown = json_body(res).await;
    assert_eq!(shown["count"], 0, "waiting for an admin");

    let res = send(
        &e.router,
        json(
            "PUT",
            &format!("/api/provider/reviews/{review}/reply"),
            Some(&provider),
            r#"{"reply":"Cảm ơn chú"}"#,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let req = Request::get("/api/provider/reviews")
        .header(header::AUTHORIZATION, format!("Bearer {provider}"))
        .body(Body::empty())
        .unwrap();
    let mine = json_body(send(&e.router, req).await).await;
    assert_eq!(mine["items"][0]["reply"], "Cảm ơn chú");
    assert_eq!(mine["items"][0]["is_visible"], false);
    assert_eq!(mine["total"], 1);

    e.t.db
        .execute_unprepared("UPDATE reviews SET is_visible = TRUE")
        .await
        .unwrap();
    let res = send(
        &e.router,
        get_page(&format!("/api/explore/providers/{business}/reviews"), None),
    )
    .await;
    let shown = json_body(res).await;
    assert_eq!(shown["count"], 1);
    assert_eq!(shown["average"], 5.0);
    assert_eq!(shown["items"][0]["reply"], "Cảm ơn chú");
    assert!(
        shown["items"][0].get("reviewer_phone_hash").is_none(),
        "no phone, not even hashed"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_review_that_looks_like_the_providers_own_is_flagged() {
    let e = env_with_origin().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let business = business_of(&e, "nongdan").await;
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    let admin = install_admin(&e).await;

    // A request from the provider's own phone, answered and reviewed at once.
    let own = e
        .state
        .usecases
        .explore
        .contact_request(ContactInput {
            business_id: business,
            topic: "Phun thuốc".into(),
            requester_name: "Khách".into(),
            requester_phone: "0901234567".into(),
            note: None,
            service_id: None,
            product_id: None,
        })
        .await
        .unwrap()
        .id;
    let res = send(
        &e.router,
        post_form(
            &format!("/provider/inquiries/{own}/status"),
            &[("status", "contacted")],
            Some(&provider),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let mut form = review_form(business, "5");
    form[2].1 = "0901234567".into();
    let res = send(&e.router, post("/reviews", &form, None)).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let list = body_text(send(&e.router, get_page("/admin/reviews", Some(&admin))).await).await;
    assert!(list.contains("gửi từ chính số điện thoại của nhà cung cấp"));
    assert!(list.contains("Viết chưa tới một giờ sau yêu cầu liên hệ"));

    // A farmer's review a day after the request carries no sign.
    let farmer = inquiry_to(&e, business).await;
    send(
        &e.router,
        post_form(
            &format!("/provider/inquiries/{farmer}/status"),
            &[("status", "closed")],
            Some(&provider),
        ),
    )
    .await;
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE inquiries SET created_on = now() - interval '1 day' WHERE id = {farmer}"
        ))
        .await
        .unwrap();
    let res = send(
        &e.router,
        post("/reviews", &review_form(business, "4"), None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let list = body_text(send(&e.router, get_page("/admin/reviews", Some(&admin))).await).await;
    assert_eq!(
        list.matches("Viết chưa tới một giờ sau yêu cầu liên hệ")
            .count(),
        1
    );
    assert_eq!(list.matches("gửi từ chính số điện thoại").count(), 1);
    e.cleanup().await;
}
