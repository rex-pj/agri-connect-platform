//! Error rendering through the real router. None of these paths may touch the
//! database, so the state's connection is disconnected on purpose.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use domain::models::role::UserRole;
use integration_tests::{json_body, router_without_db, send, token_service};

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn unknown_route_is_a_coded_vietnamese_404() {
    let res = send(&router_without_db(), get("/api/does-not-exist")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = json_body(res).await;
    assert_eq!(body["code"], "route_not_found");
    assert_eq!(body["message"], "Không tìm thấy địa chỉ này.");
}

#[tokio::test]
async fn malformed_json_body_is_a_coded_400() {
    let req = Request::post("/api/auth/tokens")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{not json"))
        .unwrap();
    let res = send(&router_without_db(), req).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["code"], "invalid_request_body");
}

#[tokio::test]
async fn non_numeric_path_param_is_a_coded_400() {
    let res = send(&router_without_db(), get("/api/explore/products/abc")).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["code"], "invalid_path_param");
}

#[tokio::test]
async fn missing_token_is_authentication_required() {
    let res = send(&router_without_db(), get("/api/users/me")).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(res).await["code"], "authentication_required");
}

#[tokio::test]
async fn forged_token_is_invalid_token() {
    let req = Request::get("/api/users/me")
        .header(header::AUTHORIZATION, "Bearer not.a.jwt")
        .body(Body::empty())
        .unwrap();
    let res = send(&router_without_db(), req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(res).await["code"], "invalid_token");
}

#[tokio::test]
async fn provider_token_on_admin_route_is_permission_denied() {
    let token = token_service().mint(42, "an", &UserRole::Provider).unwrap();
    let req = Request::get("/api/admin/stats")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let res = send(&router_without_db(), req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(res).await["code"], "permission_denied");
}

#[tokio::test]
async fn error_responses_keep_security_headers() {
    let res = send(&router_without_db(), get("/api/does-not-exist")).await;
    assert_eq!(res.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(res.headers()[header::X_FRAME_OPTIONS], "DENY");
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/json");
}

#[tokio::test]
async fn internal_error_detail_never_reaches_the_client() {
    let state = integration_tests::state(sea_orm::DatabaseConnection::default());
    let app = axum::Router::new()
        .route(
            "/boom",
            axum::routing::get(|| async {
                Err::<(), _>(domain::Error::internal(
                    "relation \"users\" does not exist (SQL)",
                ))
            }),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            web::error_response::render_errors,
        ))
        .with_state(state);

    let res = send(&app, get("/boom")).await;
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = json_body(res).await;
    assert_eq!(body["code"], "internal");
    let text = body.to_string();
    assert!(!text.contains("users") && !text.contains("SQL"), "{text}");
}
