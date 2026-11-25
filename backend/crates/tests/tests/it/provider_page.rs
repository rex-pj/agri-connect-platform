//! Provider detail and its contact form, through the real router: canonical
//! URLs, Post/Redirect/Get with a one-time flash, and errors re-shown in place.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{
    body_text, env_with_origin, form_body, get_page, register_provider, send, Env, TEST_ORIGIN,
};
use sea_orm::{ConnectionTrait, Statement};

fn post_contact(form: &str, origin: &str) -> Request<Body> {
    Request::post("/contact")
        .header(header::ACCEPT, "text/html")
        .header(header::ORIGIN, origin)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .unwrap()
}

async fn env() -> Env {
    env_with_origin().await
}

/// A provider "Vườn nongdan" with one service; returns (business id, canonical URL).
async fn provider(e: &Env) -> (i32, String) {
    register_provider(&e.state, "nongdan", "0901234567").await;
    let row =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "UPDATE businesses SET address = 'Khu phố 3, phường Cai Lậy, Đồng Tháp',
                 description = 'Múc bùn, nạo vét kênh mương.'
             WHERE name = 'Vườn nongdan' RETURNING id",
            ))
            .await
            .unwrap()
            .unwrap();
    let id: i32 = row.try_get("", "id").unwrap();
    e.t.db
        .execute_unprepared(&format!(
            "INSERT INTO services (business_id, service_type_id, name, price_note)
             SELECT {id}, id, 'Múc bùn ao', '300.000 ₫/giờ' FROM service_types
             ORDER BY display_order LIMIT 1"
        ))
        .await
        .unwrap();
    (id, format!("/providers/{id}/vuon-nongdan"))
}

async fn inquiries(e: &Env) -> i64 {
    let row =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "SELECT count(*) AS n FROM inquiries",
            ))
            .await
            .unwrap()
            .unwrap();
    row.try_get("", "n").unwrap()
}

async fn service_of(e: &Env, business_id: i32) -> i32 {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            format!("SELECT id FROM services WHERE business_id = {business_id}"),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap()
}

/// The form as the page sends it: the service select, no typed topic.
fn valid_form(id: i32, service_id: i32) -> String {
    let id = id.to_string();
    let service_id = service_id.to_string();
    form_body(&[
        ("business_id", &id),
        ("requester_phone", "0912345678"),
        ("service_id", &service_id),
        ("requester_name", "Nguyễn Văn A"),
        ("note", "Ao 500 m2"),
    ])
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_provider_page_has_its_services_a_call_button_and_the_form() {
    let e = env().await;
    let (id, canonical) = provider(&e).await;

    for uri in [
        format!("/providers/{id}"),
        format!("/providers/{id}/sai-ten"),
    ] {
        let res = send(&e.router, get_page(&uri, None)).await;
        assert_eq!(res.status(), StatusCode::MOVED_PERMANENTLY, "{uri}");
        assert_eq!(res.headers()[header::LOCATION], canonical.as_str());
    }

    let res = send(&e.router, get_page(&canonical, None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_text(res).await;
    assert!(html.contains("<title>Vườn nongdan | Agri-Connect</title>"));
    assert!(html.contains(r#"<meta name="description" content="Vườn nongdan ở "#));
    assert!(html.contains(r#". Múc bùn, nạo vét kênh mương.""#));
    assert!(html.contains("Khu phố 3, phường Cai Lậy, Đồng Tháp"));
    assert!(html.contains("Múc bùn ao") && html.contains("300.000 ₫/giờ"));
    assert!(html.contains(r#"href="tel:0901234567""#));
    // The form works without JS: a plain POST to /contact, reached by anchor.
    assert!(html.contains(r#"action="/contact""#));
    assert!(html.contains(&format!(r#"name="business_id" value="{id}""#)));
    assert!(html.contains(r##"href="#lien-he""##) && html.contains(r#"id="lien-he""#));
    let service_id = service_of(&e, id).await;
    assert!(html.contains(r#"name="service_id""#));
    assert!(html.contains(&format!(
        r#"<option value="{service_id}">Múc bùn ao</option>"#
    )));
    assert!(!html.contains("Đã gửi yêu cầu"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_sent_form_redirects_and_thanks_the_visitor_once() {
    let e = env().await;
    let (id, canonical) = provider(&e).await;
    let service_id = service_of(&e, id).await;

    let res = send(
        &e.router,
        post_contact(&valid_form(id, service_id), TEST_ORIGIN),
    )
    .await;
    // 303: reloading the thank-you page must not post the form again.
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], canonical.as_str());
    let flash = res.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(flash.starts_with("flash=contact_sent;"), "{flash}");
    assert_eq!(inquiries(&e).await, 1);
    // The inquiry names the service, and its name is the topic.
    let row =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "SELECT service_id, topic FROM inquiries",
            ))
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        row.try_get::<Option<i32>>("", "service_id").unwrap(),
        Some(service_id)
    );
    assert_eq!(row.try_get::<String>("", "topic").unwrap(), "Múc bùn ao");

    let res = send(&e.router, get_page(&canonical, Some("flash=contact_sent"))).await;
    let clears = res.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        clears.contains("Max-Age=0"),
        "the flash is used up: {clears}"
    );
    assert!(body_text(res).await.contains("Đã gửi yêu cầu!"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_rejected_form_comes_back_with_messages_and_what_was_typed() {
    let e = env().await;
    let (id, _) = provider(&e).await;
    let id_text = id.to_string();
    let form = form_body(&[
        ("business_id", &id_text),
        ("requester_phone", "123"),
        ("topic", ""),
        ("requester_name", "<script>alert(1)</script>"),
        ("note", ""),
    ]);

    let res = send(&e.router, post_contact(&form, TEST_ORIGIN)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Vui lòng kiểm tra lại thông tin."));
    assert!(
        html.contains("Vui lòng chọn dịch vụ cần liên hệ."),
        "topic message"
    );
    assert!(
        html.contains(r#"id="cs_phone-error""#),
        "phone message linked to its field"
    );
    assert!(html.contains(r#"value="123""#), "typed phone kept");
    // Echoed back escaped, never as markup.
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!html.contains("<script>alert(1)"));
    // The sheet opens by itself with JS.
    assert!(html.contains(r#"data-open="true""#));
    assert_eq!(inquiries(&e).await, 0);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn contact_for_a_missing_provider_or_from_another_site_is_refused() {
    let e = env().await;
    let (id, _) = provider(&e).await;
    let service_id = service_of(&e, id).await;

    let res = send(
        &e.router,
        post_contact(&valid_form(999_999, service_id), TEST_ORIGIN),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(body_text(res)
        .await
        .contains("Không tìm thấy nhà cung cấp."));

    let res = send(
        &e.router,
        post_contact(&valid_form(id, service_id), "https://evil.example"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(inquiries(&e).await, 0);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_inquiry_cannot_name_another_providers_service() {
    let e = env().await;
    let (id, _) = provider(&e).await;
    let theirs = service_of(&e, id).await;
    register_provider(&e.state, "nguoikhac", "0907654321").await;
    let other: i32 =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "SELECT id FROM businesses WHERE name = 'Vườn nguoikhac'",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "id")
            .unwrap();

    // The JSON API, which used to store any service id it was given.
    let body = serde_json::json!({
        "business_id": other,
        "service_id": theirs,
        "topic": "Múc bùn",
        "requester_name": "Nguyễn Văn A",
        "requester_phone": "0912345678",
    });
    let req = Request::post("/api/explore/contact")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let json = integration_tests::json_body(res).await;
    assert_eq!(
        json["field_errors"]["service_id"][0],
        "Nhà cung cấp này không còn dịch vụ đó. Vui lòng chọn lại."
    );
    assert_eq!(inquiries(&e).await, 0);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_missing_provider_page_is_a_404() {
    let e = env().await;
    for uri in ["/providers/999999/x", "/providers/khong-so"] {
        let res = send(&e.router, get_page(uri, None)).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{uri}");
    }
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn one_address_sends_one_provider_only_a_few_contact_requests_a_day() {
    let e = env().await;
    let (id, url) = provider(&e).await;
    register_provider(&e.state, "nguoikhac", "0907654321").await;
    let other: i32 =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "SELECT id FROM businesses WHERE name = 'Vườn nguoikhac'",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "id")
            .unwrap();
    let contact = |business_id: i32| {
        let body = serde_json::json!({
            "business_id": business_id,
            "topic": "Múc bùn",
            "requester_name": "Nguyễn Văn A",
            "requester_phone": "0912345678",
        });
        Request::post("/api/explore/contact")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let cap = application::constants::CONTACT_REQUESTS_PER_CLIENT_AND_PROVIDER;
    for _ in 0..cap {
        assert_eq!(
            send(&e.router, contact(id)).await.status(),
            StatusCode::CREATED
        );
    }
    let res = send(&e.router, contact(id)).await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        integration_tests::json_body(res).await["code"],
        "contact_daily_limit"
    );
    assert_eq!(
        inquiries(&e).await,
        i64::from(cap),
        "the refused one is not stored"
    );

    // The page form says so in the sheet, which stays open.
    let form = form_body(&[
        ("business_id", &id.to_string()),
        ("requester_name", "Nguyễn Văn A"),
        ("requester_phone", "0912345678"),
        ("topic", "Múc bùn"),
    ]);
    let res = send(&e.router, post_contact(&form, TEST_ORIGIN)).await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    let html = body_text(res).await;
    assert!(html.contains("gửi nhiều yêu cầu liên hệ"), "{url}");

    // Another provider, or another address, is not held back by it.
    assert_eq!(
        send(&e.router, contact(other)).await.status(),
        StatusCode::CREATED
    );
    let res = integration_tests::send_from(&e.router, contact(id), [203, 0, 113, 9]).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    e.cleanup().await;
}
