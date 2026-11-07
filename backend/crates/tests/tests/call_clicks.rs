//! Click-to-call counts: POST /api/explore/calls from a call button, the
//! business looked up from what was called (a call or a Zalo chat), and the
//! counts on the provider's dashboard and the admin dashboard. Database tests
//! need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use infrastructure::entities::{call_click, service};
use integration_tests::{
    body_text, categories_of, env_with_origin, get_page, install_admin, json_body,
    register_provider, send, sign_in, Env, TEST_ORIGIN,
};
use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

fn tap(kind: &str, id: i64, origin: &str) -> Request<Body> {
    Request::post("/api/explore/calls")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, origin)
        .body(Body::from(format!(r#"{{"kind":"{kind}","id":{id}}}"#)))
        .unwrap()
}

async fn clicks(e: &Env) -> Vec<call_click::Model> {
    call_click::Entity::find().all(&e.t.db).await.unwrap()
}

/// The provider "nongdan" with one service; returns (token, service id).
async fn provider_with_service(e: &Env) -> (String, i64) {
    let token = register_provider(&e.state, "nongdan", "0901234567").await;
    let category = categories_of(e, "service").await[0].0;
    let req = Request::post("/api/provider/services")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, TEST_ORIGIN)
        .body(Body::from(format!(
            r#"{{"category_id":{category},"name":"Múc bùn"}}"#
        )))
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    (token, json_body(res).await["id"].as_i64().unwrap())
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_tap_is_counted_for_the_business_of_what_was_called() {
    let e = env_with_origin().await;
    let (_, service_id) = provider_with_service(&e).await;

    let res = send(&e.router, tap("service", service_id, TEST_ORIGIN)).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let stored = clicks(&e).await;
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].service_id, Some(service_id as i32));
    let business_id = stored[0].business_id;

    let res = send(&e.router, tap("provider", business_id.into(), TEST_ORIGIN)).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert_eq!(clicks(&e).await.len(), 2);

    // Only what farmers can see is counted.
    let res = send(&e.router, tap("ads", 1, TEST_ORIGIN)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["code"], "invalid_call_target");
    service::Entity::update_many()
        .col_expr(service::Column::IsActive, Expr::value(false))
        .filter(service::Column::Id.eq(service_id as i32))
        .exec(&e.t.db)
        .await
        .unwrap();
    let res = send(&e.router, tap("service", service_id, TEST_ORIGIN)).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let res = send(&e.router, tap("product", 999_999, TEST_ORIGIN)).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    // Another site cannot inflate the count.
    let res = send(
        &e.router,
        tap("provider", business_id.into(), "https://evil.example"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(call_click::Entity::find().count(&e.t.db).await.unwrap(), 2);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn call_buttons_carry_the_beacon_and_the_counts_are_shown() {
    let e = env_with_origin().await;
    let (_, service_id) = provider_with_service(&e).await;

    let listing = body_text(send(&e.router, get_page("/services", None)).await).await;
    assert!(listing.contains(r#"x-data="callBeacon" x-on:click="count""#));
    assert!(listing.contains(r#"data-call-url="/api/explore/calls""#));
    assert!(listing.contains(&format!(
        r#"data-call-kind="service" data-call-id="{service_id}""#
    )));
    assert!(
        listing.contains(r#"href="tel:0901234567""#),
        "still a plain tel: link"
    );

    send(&e.router, tap("service", service_id, TEST_ORIGIN)).await;
    send(&e.router, tap("service", service_id, TEST_ORIGIN)).await;

    let cookie = sign_in(&e, "nongdan", "matkhau123").await;
    let dashboard =
        body_text(send(&e.router, get_page("/provider/dashboard", Some(&cookie))).await).await;
    assert!(dashboard.contains("2 lượt gọi và nhắn Zalo"));

    let admin = install_admin(&e).await;
    let stats = body_text(send(&e.router, get_page("/admin/dashboard", Some(&admin))).await).await;
    assert!(stats.contains("Lượt gọi + Zalo (30 ngày)"));
    e.cleanup().await;
}

fn zalo_tap(kind: &str, id: i64, channel: &str) -> Request<Body> {
    Request::post("/api/explore/calls")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, TEST_ORIGIN)
        .body(Body::from(format!(
            r#"{{"kind":"{kind}","id":{id},"channel":"{channel}"}}"#
        )))
        .unwrap()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn zalo_taps_are_leads_of_their_own_channel_and_add_up_by_week() {
    let e = env_with_origin().await;
    let (_, service_id) = provider_with_service(&e).await;

    // The detail page offers Zalo beside the call button, counted the same way.
    let res = send(
        &e.router,
        get_page(&format!("/services/{service_id}"), None),
    )
    .await;
    let canonical = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    let page = body_text(send(&e.router, get_page(&canonical, None)).await).await;
    assert!(page.contains(r#"href="https://zalo.me/0901234567""#));
    assert!(page.contains(r#"data-call-channel="zalo""#));

    let res = send(&e.router, zalo_tap("service", service_id, "zalo")).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    send(&e.router, tap("service", service_id, TEST_ORIGIN)).await;
    let res = send(&e.router, zalo_tap("service", service_id, "sms")).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let mut channels: Vec<String> = clicks(&e).await.into_iter().map(|c| c.channel).collect();
    channels.sort();
    assert_eq!(channels, ["call", "zalo"]);

    let admin = install_admin(&e).await;
    let stats = body_text(send(&e.router, get_page("/admin/dashboard", Some(&admin))).await).await;
    assert!(stats.contains("Liên hệ theo tuần"));
    // This week: one call, one Zalo chat, no request; two in all.
    assert!(
        stats.contains(
            r#"<td class="text-end">1</td><td class="text-end">1</td><td class="text-end">0</td><td class="text-end fw-bold">2</td>"#
        ),
        "{stats}"
    );
    e.cleanup().await;
}
