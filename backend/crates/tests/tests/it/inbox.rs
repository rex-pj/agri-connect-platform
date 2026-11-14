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
            product_id: None,
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
    let audited: String =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                format!(
                    "SELECT action || ': ' || reason AS line FROM audit_logs \
                     WHERE entity_type = 'inquiry' AND entity_id = {id}"
                ),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "line")
            .unwrap();
    assert_eq!(audited, "set_inquiry_status: contacted");

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

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_new_request_is_emailed_to_the_provider_and_not_audited() {
    let e = env_with_origin().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let business = business_of(&e, "nongdan").await;

    // An address nobody confirmed is never written to: a sign-up with a
    // stranger's address must not make us email them.
    inquiry_to(&e, business).await;
    assert!(e.mail.inquiries(1).await.is_empty());

    integration_tests::confirm_email(&e, "nongdan").await;
    let id = inquiry_to(&e, business).await;

    let sent = e.mail.inquiries(1).await;
    assert_eq!(sent.len(), 1);
    let (to, notice) = &sent[0];
    assert_eq!(to, "nongdan@example.vn", "the managing account");
    assert_eq!(notice.business_name, "Vườn nongdan");
    assert_eq!(notice.requester_phone, "0912345678");
    assert_eq!(notice.topic, "Múc bùn ao");
    assert_eq!(notice.inbox_path, "/provider/inquiries");

    // A visitor's request is not an admin action.
    let audited: i64 =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                format!("SELECT count(*) AS n FROM audit_logs WHERE entity_id = {id}"),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "n")
            .unwrap();
    assert_eq!(audited, 0);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn without_a_live_account_the_business_address_is_used_or_nobody() {
    let e = env_with_origin().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let business = business_of(&e, "nongdan").await;
    integration_tests::confirm_email(&e, "nongdan").await;
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE users SET is_active = FALSE;              UPDATE businesses SET email = 'vuon@example.vn' WHERE id = {business}"
        ))
        .await
        .unwrap();

    // Its account is deactivated: nobody to tell, not even the address the
    // account typed for the business.
    inquiry_to(&e, business).await;
    assert!(e.mail.inquiries(1).await.is_empty());

    // A business with no account (an admin listed it): its own address.
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE businesses SET owner_id = NULL WHERE id = {business}"
        ))
        .await
        .unwrap();
    inquiry_to(&e, business).await;
    let sent = e.mail.inquiries(1).await;
    assert_eq!(
        sent.iter().map(|(to, _)| to.as_str()).collect::<Vec<_>>(),
        ["vuon@example.vn"]
    );
    e.cleanup().await;
}
