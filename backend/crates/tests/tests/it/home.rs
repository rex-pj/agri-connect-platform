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
async fn the_home_page_features_services_then_products_with_working_links() {
    let e = Env::new().await;
    let admin = first_admin(&e.state).await;
    e.state.usecases.setup.load_demo(&admin).await.unwrap();
    let featured = e
        .state
        .usecases
        .explore
        .get_explore_data(None)
        .await
        .unwrap();
    let product = featured.featured_products.first().expect("demo products");
    let service = featured.featured_services.first().expect("demo services");
    // One place per provider, and only verified ones.
    let mut providers: Vec<i32> = featured
        .featured_services
        .iter()
        .map(|s| s.business_id)
        .collect();
    let featured_count = providers.len();
    providers.sort_unstable();
    providers.dedup();
    assert_eq!(providers.len(), featured_count, "one service per provider");
    assert!(featured
        .featured_services
        .iter()
        .all(|s| s.business_verified));

    let html = html(&e.router, page(None)).await;
    assert!(html.contains("<title>Agri-Connect — Nền tảng nông nghiệp"));
    assert!(html.contains(r#"<meta name="description" content="Tìm dịch vụ và vật tư"#));
    assert!(html.contains("Dịch vụ nổi bật"));
    // Four verified providers with services in the demo (every sector): no
    // narrowing modifier.
    assert_eq!(featured_count, 4);
    assert!(!html.contains("service-grid--1") && !html.contains("service-grid--2"));
    assert!(!html.contains("provider-card hover-lift"), "no shop block");
    let service_href = format!("/services/{}/{}", service.id, slugify(&service.name));
    assert!(html.contains(&service_href), "missing {service_href}");
    assert!(html.contains(&format!("tel:{}", service.business_phone)));
    // Canonical detail links end in the slug.
    let product_href = format!("/supplies/{}/{}", product.id, slugify(&product.name));
    assert!(html.contains(&product_href), "missing {product_href}");
    // Service category tiles first, then product ones, then the shortcuts.
    let service_tile = html.find("/services/category/").expect("service tiles");
    let product_tile = html.find("/supplies/category/").expect("product tiles");
    assert!(service_tile < product_tile, "services lead");
    assert!(html.contains(r#"href="/explore""#) && html.contains(r#"href="/providers""#));
    assert!(!html.contains("Nền tảng đang khởi động"));
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_empty_platform_shows_a_way_forward() {
    let e = Env::new().await;
    let html = html(&e.router, page(None)).await;
    assert!(html.contains("Nền tảng đang khởi động"));
    assert!(html.contains("Tìm nhà cung cấp gần tôi"));
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
    assert!(
        !html.contains(r#"x-data="profileMenu""#),
        "no session, no account menu"
    );
    // The footer is part of the frame, with the shop directory and this year.
    assert!(html.contains(r#"class="app-footer""#), "footer");
    assert!(html.contains(r#"href="/providers""#));
    let year = web::views::vn_year(chrono::Utc::now());
    assert!(html.contains(&format!("© {year} Agri-Connect")));
    // No session: no account links; no SITE_SUPPORT_* set: no help lines.
    assert!(!html.contains("footer-account"));
    assert!(!html.contains("Cần giúp đỡ?"));
}
