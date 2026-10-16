//! Product detail: canonical URLs, the rendered page, and missing products.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, Response, StatusCode},
};
use integration_tests::{create_product, put_photo, register_provider, send, Env};
use sea_orm::ConnectionTrait;
use web::views::PAGE_CSP;

const NAME: &str = "Gạo Thơm ST25 An Giang";
const SLUG: &str = "gao-thom-st25-an-giang";

fn page(uri: &str) -> Request<Body> {
    Request::get(uri)
        .header(header::ACCEPT, "text/html")
        .body(Body::empty())
        .unwrap()
}

async fn text(res: Response<Body>) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// A provider with one product; returns (token, product id).
async fn seeded(e: &Env, body: &str) -> (String, i32) {
    let token = register_provider(&e.state, "nongdan", "0901234567").await;
    let id = create_product(e, &token, body).await;
    (token, id)
}

fn rice() -> String {
    format!(
        r#"{{"category_id":4,"name":"{NAME}","price":28000,"unit":"kg",
            "description":"Gạo thơm dẻo.\nĐóng bao 5 kg."}}"#
    )
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn every_other_spelling_of_the_url_moves_permanently_to_the_canonical_one() {
    let e = Env::new().await;
    let (_, id) = seeded(&e, &rice()).await;
    let canonical = format!("/products/{id}/{SLUG}");

    for uri in [
        format!("/products/{id}"),
        format!("/products/{id}/ten-cu-truoc-khi-doi"),
        format!("/products/0{id}/{SLUG}"),
    ] {
        let res = send(&e.router, page(&uri)).await;
        // 301, not 308: the status old crawlers and link previews understand.
        assert_eq!(res.status(), StatusCode::MOVED_PERMANENTLY, "{uri}");
        assert_eq!(res.headers()[header::LOCATION], canonical.as_str(), "{uri}");
    }
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_detail_page_shows_price_description_and_a_call_button() {
    let e = Env::new().await;
    let (_, id) = seeded(&e, &rice()).await;

    let res = send(&e.router, page(&format!("/products/{id}/{SLUG}"))).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_SECURITY_POLICY], PAGE_CSP);
    let html = text(res).await;
    assert!(html.contains(&format!("<title>{NAME} — Agri-Connect</title>")));
    // The description on one line for search snippets...
    assert!(html.contains(r#"<meta name="description" content="Gạo thơm dẻo. Đóng bao 5 kg.">"#));
    // ...and with the seller's line break on the page.
    assert!(html.contains("Gạo thơm dẻo.\nĐóng bao 5 kg."));
    assert!(html.contains("28.000 ₫") && html.contains("/kg"));
    assert!(html.contains(r#"href="tel:0901234567""#));
    assert!(html.contains("Vườn nongdan"));
    // The way to the seller's page and its contact form.
    let link = html
        .split(r#"href="/providers/"#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default();
    assert!(link.ends_with("/vuon-nongdan"), "provider link: {link:?}");
    assert!(!html.contains("product-detail--photo"), "no photo yet");
    assert!(!html.contains("{{") && !html.contains("{%"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_product_with_a_photo_shows_the_full_size_image() {
    let e = Env::new().await;
    let (token, id) = seeded(&e, &rice()).await;
    let file = put_photo(&e, id, &token).await;

    let html = text(send(&e.router, page(&format!("/products/{id}/{SLUG}"))).await).await;
    assert!(html.contains("product-detail--photo"));
    // The main image, not the thumbnail the cards use.
    assert!(html.contains(&format!(r#"src="/files/{file}""#)), "{html}");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn without_a_description_the_meta_tag_says_who_sells_it() {
    let e = Env::new().await;
    let (_, id) = seeded(&e, r#"{"category_id":4,"name":"Sầu riêng"}"#).await;
    let html = text(send(&e.router, page(&format!("/products/{id}/sau-rieng"))).await).await;
    assert!(html.contains("Mua Sầu riêng từ Vườn nongdan trên Agri-Connect."));
    assert!(html.contains("Liên hệ giá"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn missing_hidden_and_mistyped_products_are_a_404_page() {
    let e = Env::new().await;
    let (_, id) = seeded(&e, &rice()).await;
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE products SET is_active = FALSE WHERE id = {id}"
        ))
        .await
        .unwrap();

    for uri in [
        format!("/products/{id}/{SLUG}"), // deactivated
        "/products/999999/x".to_owned(),
        "/products/khong-phai-so".to_owned(),
        "/products/99999999999999/x".to_owned(), // overflows i32
    ] {
        let res = send(&e.router, page(&uri)).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{uri}");
        assert!(
            text(res).await.contains("Không tìm thấy sản phẩm."),
            "{uri}"
        );
    }
    e.cleanup().await;
}
