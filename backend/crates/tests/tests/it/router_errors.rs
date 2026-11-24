//! Error rendering through the real router. None of these paths may touch the
//! database, so the state's connection is disconnected on purpose.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{json_body, router_without_db, send};

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
async fn error_responses_keep_security_headers() {
    let res = send(&router_without_db(), get("/api/does-not-exist")).await;
    assert_eq!(res.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(res.headers()[header::X_FRAME_OPTIONS], "DENY");
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/json");
}

#[tokio::test]
async fn internal_error_detail_never_reaches_the_client() {
    let state = integration_tests::state(integration_tests::unreachable_db());
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

/// A router that also lets the Zalo Mini App's host call the API.
fn router_with_api_origin(api: &str) -> axum::Router {
    integration_tests::router_with(
        integration_tests::state(integration_tests::unreachable_db()),
        web::routings::app_routing::RouterConfig {
            allowed_origins: vec![integration_tests::TEST_ORIGIN.to_owned()],
            api_origins: vec![api.to_owned()],
            ..Default::default()
        },
    )
}

#[tokio::test]
async fn an_api_origin_writes_only_without_a_session_cookie() {
    const MINI_APP: &str = "https://h5.zdn.vn";
    let router = router_with_api_origin(MINI_APP);
    let tap = |cookie: Option<&str>| {
        let mut req = Request::post("/api/explore/calls")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, MINI_APP);
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, c);
        }
        req.body(Body::from(r#"{"kind":"ads","id":1}"#)).unwrap()
    };

    // No cookie: the request gets through to the handler, which refuses the
    // made-up target.
    let res = send(&router, tap(None)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["code"], "invalid_call_target");

    // With a session cookie of ours, a page on that host is another site.
    for cookie in ["auth_token=x", "refresh_token=x"] {
        let res = send(&router, tap(Some(cookie))).await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "{cookie}");
        assert_eq!(json_body(res).await["code"], "cross_origin_request");
    }
}

#[tokio::test]
async fn a_clients_request_id_is_kept_only_when_it_is_a_plain_id() {
    let id_of = |res: &axum::http::Response<Body>| {
        res.headers()["x-request-id"].to_str().unwrap().to_owned()
    };
    let with_id = |id: &str| {
        Request::get("/health/live")
            .header("x-request-id", id)
            .body(Body::empty())
            .unwrap()
    };
    let router = router_without_db();
    let res = send(&router, with_id("bao-loi_2026-11")).await;
    assert_eq!(id_of(&res), "bao-loi_2026-11");
    for odd in ["a b", "<script>", &"x".repeat(200)] {
        let res = send(&router, with_id(odd)).await;
        let id = id_of(&res);
        assert_ne!(id, odd);
        assert_eq!(id.len(), 36, "a fresh id: {id}");
    }
}
