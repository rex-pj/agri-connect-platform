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
