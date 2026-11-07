//! The Zalo Mini App's token conversions. Without ZALO_APP_SECRET they
//! answer `zalo_not_configured` (503), so the Mini App falls back to an area
//! and a typed phone. Database tests need TEST_DATABASE_URL and
//! `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{env_with_origin, json_body, send, TEST_ORIGIN};

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn without_a_secret_the_mini_app_is_told_to_fall_back() {
    let e = env_with_origin().await;
    for path in ["/api/zalo/location", "/api/zalo/phone"] {
        let res = send(
            &e.router,
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, TEST_ORIGIN)
                .body(Body::from(r#"{"access_token":"a","token":"b"}"#))
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(json_body(res).await["code"], "zalo_not_configured");
    }
}
