//! The home page and the app frame, rendered by Rust from real data.
//! Run with TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use integration_tests::{first_admin, register_provider, router_without_db, send, Env};
use web::{app_state::AppState, slug::slugify, views::PAGE_CSP};

fn page(cookie: Option<&str>) -> Request<Body> {
    let mut req = Request::get("/").header(header::ACCEPT, "text/html");
    if let Some(token) = cookie {
        req = req.header(header::COOKIE, format!("auth_token={token}"));
    }
    req.body(Body::empty()).unwrap()
}

async fn html(router: &Router, req: Request<Body>) -> String {
    let res = send(router, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_SECURITY_POLICY], PAGE_CSP);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(
        !html.contains("{{") && !html.contains("{%"),
        "unrendered template syntax"
    );
    html
}

async fn provider_token(state: &AppState) -> String {
    register_provider(state, "nongdan", "0901234567").await
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_home_page_lists_featured_products_and_providers_with_working_links() {
    let e = Env::new().await;
    let admin = first_admin(&e.state).await;
    e.state.usecases.setup.load_demo(&admin).await.unwrap();
    let featured = e.state.usecases.explore.get_explore_data().await.unwrap();
    let product = featured.featured_products.first().expect("demo products");
    let provider = featured.featured_providers.first().expect("demo providers");

    let html = html(&e.router, page(None)).await;
    assert!(html.contains("<title>Agri-Connect — Nền tảng nông nghiệp"));
    assert!(html.contains(r#"<meta name="description" content="Tìm nhà cung cấp"#));
    // Canonical detail links with the same slug SvelteKit builds.
    let product_href = format!("/products/{}/{}", product.id, slugify(&product.name));
    assert!(html.contains(&product_href), "missing {product_href}");
    assert!(html.contains(&format!("tel:{}", product.vendor_phone)));
    let provider_href = format!("/providers/{}/{}", provider.id, slugify(&provider.name));
    assert!(html.contains(&provider_href), "missing {provider_href}");
    // Category tiles from the catalogue, then the two shortcuts.
    assert!(
        html.contains("/products/category/"),
        "product category tiles"
    );
    assert!(html.contains(r#"href="/services""#) && html.contains(r#"href="/explore""#));
    assert!(!html.contains("Nền tảng đang khởi động"));
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_empty_platform_shows_a_way_forward() {
    let e = Env::new().await;
    let html = html(&e.router, page(None)).await;
    assert!(html.contains("Nền tảng đang khởi động"));
    assert!(html.contains("Tìm thợ gần tôi"));
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_frame_follows_the_session() {
    let e = Env::new().await;

    let anonymous = html(&e.router, page(None)).await;
    assert!(anonymous.contains(r#"href="/login""#));
    assert!(!anonymous.contains("Đăng xuất"));

    let token = provider_token(&e.state).await;
    let provider = html(&e.router, page(Some(&token))).await;
    assert!(provider.contains(r#"href="/provider/dashboard""#));
    assert!(provider.contains("Đăng xuất"));
    assert!(!provider.contains(r#"href="/admin/dashboard""#));

    let admin = first_admin(&e.state).await;
    let token = e
        .state
        .usecases
        .auth
        .login(admin.username, "matkhau123".into())
        .await
        .unwrap()
        .token;
    let admin = html(&e.router, page(Some(&token))).await;
    assert!(admin.contains(r#"href="/admin/dashboard""#));
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_stale_session_cookie_shows_the_page_signed_out() {
    // An expired cookie must never lock anyone out of the home page.
    let e = Env::new().await;
    let html = html(&e.router, page(Some("not.a.valid-token"))).await;
    assert!(html.contains(r#"href="/login""#));
    e.t.cleanup().await;
}

#[tokio::test]
async fn error_pages_sit_in_the_app_frame_without_an_account_menu() {
    let res = send(
        &router_without_db(),
        Request::get("/khong-co")
            .header(header::ACCEPT, "text/html")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(html.contains(r#"class="bottom-nav""#), "bottom tabs");
    assert!(html.contains(r#"class="app-nav""#), "top bar");
    assert!(!html.contains("app-nav__dropdown"), "no session, no menu");
}
