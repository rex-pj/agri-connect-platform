//! Product images end to end: multipart upload through the real router, the
//! files row, the objects (in-memory storage) and cleanup by events.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use std::sync::Arc;

use application::catalogue::CATALOGUE;
use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    Router,
};
use domain::repositories::catalogue_repository_trait::CatalogueRepositoryTrait;
use image::{codecs::jpeg::JpegEncoder, Rgb, RgbImage};
use infrastructure::catalogue_repository::CatalogueRepository;
use integration_tests::{
    json_body, memory_uploads, router, router_without_db, send, state_with_uploads, TestDb,
};
use sea_orm::{ConnectionTrait, Statement};
use web::app_state::AppState;

const BOUNDARY: &str = "agri-test-boundary";

fn photo(w: u32, h: u32) -> Vec<u8> {
    let img = RgbImage::from_fn(w, h, |x, y| Rgb([(x % 256) as u8, (y % 256) as u8, 120]));
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&img)
        .unwrap();
    out
}

fn multipart(field: &str, bytes: &[u8]) -> Vec<u8> {
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"anh.jpg\"\r\n\
         Content-Type: image/jpeg\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

fn upload(product_id: i32, token: Option<&str>, body: Vec<u8>) -> Request<Body> {
    let mut req = Request::builder()
        .method(Method::PUT)
        .uri(format!("/api/provider/products/{product_id}/image"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        );
    if let Some(token) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    req.body(Body::from(body)).unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

struct Env {
    t: TestDb,
    state: AppState,
    router: Router,
}

async fn env_with(max_bytes: Option<usize>) -> Env {
    let t = TestDb::create().await;
    CatalogueRepository::new(Arc::new(t.db.clone()))
        .apply(&CATALOGUE)
        .await
        .unwrap();
    let state = state_with_uploads(t.db.clone(), memory_uploads(max_bytes));
    let router = router(state.clone());
    Env { t, state, router }
}

async fn env() -> Env {
    env_with(None).await
}

async fn provider(state: &AppState, username: &str, phone: &str) -> String {
    state
        .usecases
        .auth
        .register(
            username.into(),
            format!("{username}@example.vn"),
            "matkhau123".into(),
            None,
            format!("Vườn {username}"),
            phone.into(),
        )
        .await
        .unwrap_or_else(|e| panic!("register: {e}"))
        .token
}

async fn product(e: &Env, token: &str) -> i32 {
    let req = Request::post("/api/provider/products")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"category_id":4,"name":"Sầu riêng Ri6"}"#))
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    json_body(res).await["id"].as_i64().unwrap() as i32
}

async fn file_rows(e: &Env) -> i64 {
    let row =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "SELECT count(*) AS n FROM files",
            ))
            .await
            .unwrap()
            .unwrap();
    row.try_get::<i64>("", "n").unwrap()
}

/// Uploads a photo and returns the new file id.
async fn put_photo(e: &Env, product_id: i32, token: &str) -> String {
    let res = send(
        &e.router,
        upload(product_id, Some(token), multipart("image", &photo(64, 48))),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await["image_file_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn code(res: axum::http::Response<Body>) -> String {
    json_body(res).await["code"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn an_anonymous_upload_is_refused_before_the_body_is_read() {
    let res = send(
        &router_without_db(),
        upload(1, None, multipart("image", b"x")),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_uploaded_image_is_served_from_our_origin_and_cached_forever() {
    let e = env().await;
    let token = provider(&e.state, "nongdan", "0901234567").await;
    let id = product(&e, &token).await;
    let file = put_photo(&e, id, &token).await;

    for path in [format!("/files/{file}"), format!("/files/{file}/thumb")] {
        let res = send(&e.router, get(&path)).await;
        assert_eq!(res.status(), StatusCode::OK, "{path}");
        let h = res.headers();
        assert_eq!(h[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(
            h[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        assert_eq!(h[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(bytes.starts_with(&[0xFF, 0xD8]), "{path} is a JPEG");
    }

    // The public listing names the image too.
    let res = send(&e.router, get(&format!("/api/explore/products/{id}"))).await;
    assert_eq!(json_body(res).await["image_file_id"], file.as_str());
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_replaced_image_is_removed() {
    let e = env().await;
    let token = provider(&e.state, "nongdan", "0901234567").await;
    let id = product(&e, &token).await;
    let first = put_photo(&e, id, &token).await;
    let second = put_photo(&e, id, &token).await;
    assert_ne!(
        first, second,
        "a new image gets a new id, so caches never go stale"
    );

    let res = send(&e.router, get(&format!("/files/{first}"))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(code(res).await, "file_not_found");
    assert_eq!(file_rows(&e).await, 1);
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn deleting_a_product_removes_its_image() {
    let e = env().await;
    let token = provider(&e.state, "nongdan", "0901234567").await;
    let id = product(&e, &token).await;
    let file = put_photo(&e, id, &token).await;

    let req = Request::delete(format!("/api/provider/products/{id}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&e.router, req).await.status(), StatusCode::NO_CONTENT);

    let res = send(&e.router, get(&format!("/files/{file}/thumb"))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(file_rows(&e).await, 0);
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn nobody_can_set_the_image_of_someone_elses_product() {
    let e = env().await;
    let owner = provider(&e.state, "chuvuon", "0901234567").await;
    let other = provider(&e.state, "hangxom", "0907654321").await;
    let id = product(&e, &owner).await;

    let res = send(
        &e.router,
        upload(id, Some(&other), multipart("image", &photo(32, 32))),
    )
    .await;
    // Indistinguishable from a missing product, so ids cannot be probed.
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(code(res).await, "product_not_found");
    assert_eq!(
        file_rows(&e).await,
        0,
        "nothing stored for a refused upload"
    );
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn bad_uploads_get_a_reason_and_store_nothing() {
    let e = env_with(Some(2048)).await;
    let token = provider(&e.state, "nongdan", "0901234567").await;
    let id = product(&e, &token).await;

    let cases: [(Vec<u8>, StatusCode, &str); 5] = [
        // Over UPLOAD_MAX_BYTES (2 KB here) but within the body limit, which
        // allows for multipart framing: refused by the use case.
        (
            multipart("image", &vec![0u8; 8 * 1024]),
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_too_large",
        ),
        // Past the body limit too: refused while reading, same answer.
        (
            multipart("image", &vec![0u8; 200 * 1024]),
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_too_large",
        ),
        // A script renamed .jpg: the bytes decide, not the name or the type.
        (
            multipart("image", b"<script>alert(1)</script>"),
            StatusCode::BAD_REQUEST,
            "unsupported_image_type",
        ),
        (
            multipart("photo", &photo(8, 8)),
            StatusCode::BAD_REQUEST,
            "image_required",
        ),
        (
            multipart("image", b""),
            StatusCode::BAD_REQUEST,
            "image_required",
        ),
    ];
    for (body, status, expected) in cases {
        let res = send(&e.router, upload(id, Some(&token), body)).await;
        assert_eq!(res.status(), status, "{expected}");
        let json = json_body(res).await;
        assert_eq!(json["code"], expected);
        if expected == "file_too_large" {
            assert!(
                json["message"].as_str().unwrap().contains("1 MB"),
                "the cap is in the message: {json}"
            );
        }
    }
    assert_eq!(file_rows(&e).await, 0);
    e.t.cleanup().await;
}
