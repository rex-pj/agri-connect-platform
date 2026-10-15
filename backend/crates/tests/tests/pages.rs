//! Server-rendered pages through the real router: the error page and its
//! assets.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{router_without_db, send};
use web::views::PAGE_CSP;

fn page(uri: &str) -> Request<Body> {
    Request::get(uri)
        .header(header::ACCEPT, "text/html,application/xhtml+xml")
        .body(Body::empty())
        .unwrap()
}

async fn text(res: axum::http::Response<Body>) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn an_unknown_page_renders_the_vietnamese_error_page() {
    let res = send(&router_without_db(), page("/khong-co-trang-nay")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        res.headers()[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
    assert_eq!(res.headers()[header::CONTENT_SECURITY_POLICY], PAGE_CSP);
    let html = text(res).await;
    assert!(html.starts_with("<!doctype html>"), "{html}");
    assert!(html.contains(r#"<html lang="vi">"#));
    assert!(html.contains("Không tìm thấy địa chỉ này."));
    assert!(html.contains(r#"<meta name="robots" content="noindex">"#));
    assert!(html.contains(r#"href="/""#), "home link from url_for");
    assert!(
        !html.contains("{{") && !html.contains("{%"),
        "unrendered template syntax"
    );
}

#[tokio::test]
async fn page_csp_allows_only_our_own_scripts() {
    assert!(PAGE_CSP.contains("script-src 'self';"));
    assert!(!PAGE_CSP.contains("unsafe-eval") && !PAGE_CSP.contains("unsafe-inline"));
}

#[tokio::test]
async fn api_errors_stay_json_even_for_a_browser() {
    let res = send(&router_without_db(), page("/api/nope")).await;
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/json");
}

fn asset_urls(html: &str) -> Vec<String> {
    html.split(['"', '\''])
        .filter(|s| s.starts_with("/static/"))
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
async fn every_asset_the_layout_links_is_served_and_cached_for_a_year() {
    let router = router_without_db();
    let html = text(send(&router, page("/khong-co")).await).await;
    let urls = asset_urls(&html);
    assert!(urls.len() >= 5, "expected css + js links, got {urls:?}");
    for url in urls {
        let res = send(&router, Request::get(&url).body(Body::empty()).unwrap()).await;
        assert_eq!(res.status(), StatusCode::OK, "{url}");
        assert_eq!(
            res.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable",
            "{url}"
        );
        let ct = res.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .to_owned();
        assert!(
            ct.starts_with("text/css") || ct.starts_with("text/javascript"),
            "{url}: {ct}"
        );
    }
}

#[tokio::test]
async fn fonts_referenced_by_css_exist() {
    let router = router_without_db();
    for url in [
        "/static/vendor/fontawesome-6.5.1/webfonts/fa-solid-900.woff2",
        "/static/fonts/be-vietnam-pro/be-vietnam-pro-400-vietnamese.woff2",
        "/static/fonts/be-vietnam-pro/be-vietnam-pro-800-latin.woff2",
    ] {
        let res = send(&router, Request::get(url).body(Body::empty()).unwrap()).await;
        assert_eq!(res.status(), StatusCode::OK, "{url}");
        assert_eq!(res.headers()[header::CONTENT_TYPE], "font/woff2");
    }
}

#[tokio::test]
async fn provenance_files_are_not_served() {
    for url in [
        "/static/vendor/VERSIONS",
        "/static/vendor/bootstrap-5.3.8/LICENSE",
    ] {
        let res = send(
            &router_without_db(),
            Request::get(url).body(Body::empty()).unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{url}");
    }
}
