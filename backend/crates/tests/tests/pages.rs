//! Server-rendered pages through the real router: the error page, its assets,
//! and the transitional SvelteKit proxy.

use axum::{
    body::Body,
    http::{header, HeaderMap, Request, StatusCode},
    routing::get,
    Router,
};
use integration_tests::{router_with, router_without_db, send, state};
use sea_orm::DatabaseConnection;
use web::{legacy::LegacyProxy, routings::app_routing::RouterConfig, views::PAGE_CSP};

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

/// A stand-in for the SvelteKit server that reports what it received.
async fn fake_sveltekit() -> String {
    let app = Router::new().fallback(get(|headers: HeaderMap, uri: axum::http::Uri| async move {
        let h = |name: &str| {
            headers
                .get(name)
                .map(|v| v.to_str().unwrap().to_owned())
                .unwrap_or_default()
        };
        (
            [("connection", "close"), ("x-from", "sveltekit")],
            format!(
                "path={} host={} proto={} fwd={}",
                uri,
                h("host"),
                h("x-forwarded-proto"),
                h("x-forwarded-for")
            ),
        )
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

fn proxied_router(base: &str) -> Router {
    let proxy = LegacyProxy::new(&base.parse().unwrap(), false).unwrap();
    router_with(
        state(DatabaseConnection::default()),
        RouterConfig {
            legacy: Some(proxy),
            ..RouterConfig::default()
        },
    )
}

#[tokio::test]
async fn unmigrated_pages_are_proxied_with_the_browser_host() {
    let router = proxied_router(&fake_sveltekit().await);
    let req = Request::get("/provider/dashboard?tab=2")
        .header(header::HOST, "agri.example.vn")
        .body(Body::empty())
        .unwrap();
    let res = send(&router, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["x-from"], "sveltekit");
    assert!(
        !res.headers().contains_key("connection"),
        "hop-by-hop header leaked"
    );
    let body = text(res).await;
    assert!(body.contains("path=/provider/dashboard?tab=2"), "{body}");
    assert!(body.contains("host=agri.example.vn"), "{body}");
    assert!(body.contains("proto=http"), "{body}");
    assert!(body.contains("fwd=127.0.0.1"), "{body}");
}

#[tokio::test]
async fn api_paths_are_never_proxied() {
    let router = proxied_router(&fake_sveltekit().await);
    let res = send(
        &router,
        Request::get("/api/nope").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/json");
}

#[tokio::test]
async fn a_down_sveltekit_gives_a_503_page_not_a_hang() {
    // Port 9 (discard) on loopback refuses connections on every CI runner.
    let router = proxied_router("http://127.0.0.1:9");
    let res = send(&router, page("/provider/dashboard")).await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(text(res)
        .await
        .contains("Trang này tạm thời không truy cập được."));
}
