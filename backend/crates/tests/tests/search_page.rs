//! /search: both result kinds, link tabs that work without JS, previews with
//! "see more" links. Database tests need TEST_DATABASE_URL and
//! `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{categories_of, send, Env};
use sea_orm::ConnectionTrait;

async fn html(e: &Env, uri: &str) -> String {
    let req = Request::get(uri)
        .header(header::ACCEPT, "text/html")
        .body(Body::empty())
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::OK, "{uri}");
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// `providers` fertiliser shops (each with a service) and `products`
/// fertiliser products, all matching "phan bon".
async fn seeded(e: &Env, providers: usize, products: usize) {
    let service = categories_of(e, "service").await[0].0;
    let product = categories_of(e, "product").await[0].0;
    e.t.db
        .execute_unprepared(&format!(
            "INSERT INTO businesses (name, phone)
             SELECT 'Đại lý Phân bón ' || n, '09010000' || lpad(n::text, 2, '0')
             FROM generate_series(1, {providers}) AS n;
             INSERT INTO services (business_id, category_id, name)
             SELECT id, {service}, 'Giao phân' FROM businesses;
             INSERT INTO products (business_id, category_id, name, price)
             SELECT (SELECT min(id) FROM businesses), {product}, 'Phân bón NPK ' || n, 1000 * n
             FROM generate_series(1, {products}) AS n;"
        ))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn one_search_finds_providers_and_products_with_tab_links() {
    let e = Env::new().await;
    seeded(&e, 2, 3).await;

    let page = html(&e, "/search?q=phan+bon").await;
    assert!(page.contains("<title>Tìm kiếm “phan bon” — Agri-Connect</title>"));
    assert!(page.contains(r#"<meta name="robots" content="noindex">"#));
    assert!(page.contains("Nhà cung cấp <span class=\"text-muted fw-normal small\">(2)</span>"));
    assert!(page.contains("Sản phẩm <span class=\"text-muted fw-normal small\">(3)</span>"));
    // Tabs are real links carrying the query; "all" is current.
    assert!(page.contains(r#"href="/search?q=phan+bon&amp;tab=services""#));
    assert!(page.contains(r#"href="/search?q=phan+bon" data-tab="all" aria-current="page""#));
    assert!(
        !page.contains(" hidden "),
        "every section visible on the all tab"
    );
    // Below the preview size: no "see more".
    assert!(!page.contains("Xem thêm"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_tab_link_hides_the_other_section_without_js() {
    let e = Env::new().await;
    seeded(&e, 1, 1).await;
    let page = html(&e, "/search?q=phan+bon&tab=services").await;
    assert!(page.contains(r#"data-panel="products" hidden"#));
    assert!(!page.contains(r#"data-panel="services" hidden"#));
    assert!(page.contains(r#"data-tab="services" aria-current="page""#));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn long_results_preview_six_and_link_to_the_full_list() {
    let e = Env::new().await;
    seeded(&e, 8, 9).await;
    let page = html(&e, "/search?q=phan+bon").await;
    assert_eq!(page.matches("provider-card hover-lift").count(), 6);
    assert_eq!(page.matches("product-card hover-lift").count(), 6);
    assert!(page.contains(r#"href="/services?q=phan+bon""#));
    assert!(page.contains(r#"href="/products?q=phan+bon""#));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn no_query_asks_for_one_and_nothing_found_points_to_the_lists() {
    let e = Env::new().await;
    let empty = html(&e, "/search").await;
    assert!(empty.contains("Nhập từ khóa để tìm kiếm"));
    assert!(empty.contains("<title>Tìm kiếm — Agri-Connect</title>"));

    let none = html(&e, "/search?q=xyz&tab=bogus").await;
    assert!(none.contains("Không có nhà cung cấp phù hợp"));
    assert!(none.contains("Không có sản phẩm phù hợp"));
    // The top bar's search box stays empty here: the page has its own.
    assert!(none.contains(r#"name="q" value="" placeholder="Tìm dịch vụ"#));
    e.cleanup().await;
}
