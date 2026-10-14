//! Health probes and request ids, through the real router.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use integration_tests::{json_body, router_without_db, send};

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn live_answers_without_a_database() {
    let res = send(&router_without_db(), get("/health/live")).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["status"], "live");
}

#[tokio::test]
async fn ready_is_503_when_the_database_is_down() {
    let res = send(&router_without_db(), get("/health/ready")).await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json_body(res).await["code"], "database_unavailable");
}

#[tokio::test]
async fn the_old_health_path_is_gone() {
    // Nothing was ever deployed, so no probe depends on it (plan §8.10).
    let res = send(&router_without_db(), get("/health")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn every_response_carries_a_request_id() {
    let res = send(&router_without_db(), get("/health/live")).await;
    let id = res
        .headers()
        .get("x-request-id")
        .expect("x-request-id missing");
    assert_eq!(id.len(), 36, "expected a UUID, got {id:?}");
}

#[tokio::test]
async fn error_responses_carry_a_request_id_too() {
    let res = send(&router_without_db(), get("/api/nope")).await;
    assert!(res.headers().contains_key("x-request-id"));
}

#[tokio::test]
async fn a_client_request_id_is_echoed() {
    let req = Request::get("/health/live")
        .header("x-request-id", "caddy-123")
        .body(Body::empty())
        .unwrap();
    let res = send(&router_without_db(), req).await;
    assert_eq!(res.headers()["x-request-id"], "caddy-123");
}

fn bad_login_from(client: &str) -> Request<Body> {
    // Malformed body: rejected by the handler before it touches the database,
    // but only after the rate limiter has counted it.
    Request::post("/api/auth/tokens")
        .header("content-type", "application/json")
        .header("x-forwarded-for", client)
        .body(Body::from("{"))
        .unwrap()
}

#[tokio::test]
async fn behind_caddy_each_client_has_its_own_rate_limit() {
    let router = integration_tests::router_with(
        integration_tests::state(sea_orm::DatabaseConnection::default()),
        web::routings::app_routing::RouterConfig {
            trusted_proxy_hops: 1,
            ..Default::default()
        },
    );
    for _ in 0..10 {
        let res = send(&router, bad_login_from("1.2.3.4")).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
    let res = send(&router, bad_login_from("1.2.3.4")).await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json_body(res).await["code"], "too_many_requests");

    // Same proxy (same TCP peer), different client: not affected.
    let res = send(&router, bad_login_from("5.6.7.8")).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_forged_forwarded_for_does_not_buy_a_fresh_limit() {
    let router = integration_tests::router_with(
        integration_tests::state(sea_orm::DatabaseConnection::default()),
        web::routings::app_routing::RouterConfig {
            trusted_proxy_hops: 1,
            ..Default::default()
        },
    );
    // The attacker rotates a fake first entry; Caddy's appended entry is the same.
    for i in 0..10 {
        send(&router, bad_login_from(&format!("9.9.9.{i}, 1.2.3.4"))).await;
    }
    let res = send(&router, bad_login_from("9.9.9.99, 1.2.3.4")).await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
}
