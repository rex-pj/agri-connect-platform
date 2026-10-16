//! /provider/inquiries: the provider's own inbox of contact requests.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use application::inputs::ContactInput;
use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, env_with_origin, get_page, post_form, register_provider, send, sign_in, Env,
};
use sea_orm::{ConnectionTrait, Statement};

async fn business_of(e: &Env, username: &str) -> i32 {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            format!("SELECT id FROM businesses WHERE name = 'Vườn {username}'"),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap()
}

/// A farmer's request to `business_id`; returns its id.
async fn inquiry_to(e: &Env, business_id: i32) -> i32 {
    e.state
        .usecases
        .explore
        .contact_request(ContactInput {
            business_id,
            topic: "Múc bùn ao".into(),
            requester_name: "Nguyễn Văn A".into(),
            requester_phone: "0912345678".into(),
            note: Some("Ao 500 m2, cần trong tuần".into()),
            service_id: None,
        })
        .await
        .unwrap()
        .id
}

async fn status_of(e: &Env, id: i32) -> String {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            format!("SELECT status FROM inquiries WHERE id = {id}"),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "status")
        .unwrap()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_provider_sees_and_handles_the_requests_sent_to_them() {
    let e = env_with_origin().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let id = inquiry_to(&e, business_of(&e, "nongdan").await).await;
    let cookie = sign_in(&e, "nongdan", "matkhau123").await;

    let dashboard =
        body_text(send(&e.router, get_page("/provider/dashboard", Some(&cookie))).await).await;
    assert!(dashboard.contains(r#"href="/provider/inquiries""#));
    assert!(dashboard.contains("1 người đang chờ bạn gọi"));

    let res = send(&e.router, get_page("/provider/inquiries", Some(&cookie))).await;
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_text(res).await;
    assert!(html.contains("Nguyễn Văn A"));
    assert!(
        html.contains(r#"href="tel:0912345678""#),
        "one tap to call back"
    );
    assert!(
        html.contains("Ao 500 m2, cần trong tuần"),
        "the farmer's note"
    );
    assert!(html.contains("noindex"));

    let res = send(
        &e.router,
        post_form(
            &format!("/provider/inquiries/{id}/status"),
            &[("status", "contacted"), ("page", "1")],
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/provider/inquiries");
    assert_eq!(status_of(&e, id).await, "contacted");

    // A made-up status is refused, the stored one kept.
    let res = send(
        &e.router,
        post_form(
            &format!("/provider/inquiries/{id}/status"),
            &[("status", "da-goi")],
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(status_of(&e, id).await, "contacted");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn another_providers_requests_stay_out_of_sight_and_reach() {
    let e = env_with_origin().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    register_provider(&e.state, "nguoikhac", "0907654321").await;
    let id = inquiry_to(&e, business_of(&e, "nongdan").await).await;
    let other = sign_in(&e, "nguoikhac", "matkhau123").await;

    let html =
        body_text(send(&e.router, get_page("/provider/inquiries", Some(&other))).await).await;
    assert!(!html.contains("Nguyễn Văn A"));
    assert!(html.contains("Chưa có yêu cầu nào"));

    let res = send(
        &e.router,
        post_form(
            &format!("/provider/inquiries/{id}/status"),
            &[("status", "spam")],
            Some(&other),
        ),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "looks the same as missing"
    );
    assert_eq!(status_of(&e, id).await, "new");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn without_the_permission_there_is_no_inbox() {
    let e = env_with_origin().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    inquiry_to(&e, business_of(&e, "nongdan").await).await;
    e.t.db
        .execute_unprepared(
            "DELETE FROM role_permissions WHERE permission_id = \
             (SELECT id FROM permissions WHERE code = 'own.inquiry.manage')",
        )
        .await
        .unwrap();
    let cookie = sign_in(&e, "nongdan", "matkhau123").await;

    let res = send(&e.router, get_page("/provider/inquiries", Some(&cookie))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let dashboard =
        body_text(send(&e.router, get_page("/provider/dashboard", Some(&cookie))).await).await;
    assert!(!dashboard.contains(r#"href="/provider/inquiries""#));
    e.cleanup().await;
}
