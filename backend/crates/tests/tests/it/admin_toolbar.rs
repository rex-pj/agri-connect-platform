//! The admin lists' toolbar: every filter it offers is read from the URL,
//! kept by the page, and narrows the list (or at least never breaks it).
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::StatusCode;
use integration_tests::{
    body_text, env_with_origin, get_page, install_admin, post_form, send, Env,
};

async fn page(e: &Env, path: &str, cookie: &str) -> String {
    let res = send(&e.router, get_page(path, Some(cookie))).await;
    assert_eq!(res.status(), StatusCode::OK, "{path}");
    body_text(res).await
}

/// An admin with the demo data loaded from the demo data page.
async fn with_demo(e: &Env) -> String {
    let cookie = install_admin(e).await;
    let res = send(
        &e.router,
        post_form("/setup/seed", &[("action", "load")], Some(&cookie)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    cookie
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn every_toolbar_filter_is_read_and_keeps_the_list_working() {
    let e = env_with_origin().await;
    let admin = with_demo(&e).await;
    let lists = [
        "/admin/users?gap=unverified_email",
        "/admin/users?role_id=1",
        "/admin/providers?sector=trong_trot&rating=4",
        "/admin/providers?area=Cai%20L%E1%BA%ADy&gap=no_services",
        "/admin/providers?gap=no_products&category_id=1",
        "/admin/providers?gap=no_account",
        "/admin/services?sector=trong_trot&gap=no_image",
        "/admin/services?category_id=1&from=2020-01-01&to=2099-01-01",
        "/admin/supplies?sector=trong_trot&gap=no_price",
        "/admin/supplies?gap=no_image&fee_min=1000&fee_max=900000",
        "/admin/supplies?from=2020-01-01&to=2099-01-01&sort=name",
        "/admin/reviews?gap=stale_3&rating=3",
        "/admin/reviews?gap=stale_7&reply=none",
        "/admin/inquiries?gap=stale_3&kind=service",
        "/admin/jobs?gap=stale_7&location_id=1",
        "/admin/partners?contract=in_force",
        "/admin/partners?contract=expiring&kind=distributor",
        "/admin/partners?contract=ended",
        "/admin/partners?contract=none",
        "/admin/categories?type=service&gap=unused&sort=name",
        "/admin/categories?type=product&state=shown&sort=newest",
        "/admin/commodities?sort=name&state=hidden",
        "/admin/locations?sort=oldest&state=shown",
        "/admin/prices?tab=crops&sort=name&state=shown",
        "/admin/prices?gap=stale",
        "/admin/prices?q=Cai&from=2020-01-01&to=2099-01-01",
        "/admin/alerts?state=live&sort=name",
        "/admin/alerts?state=ended&q=m",
        "/admin/sectors?state=hidden",
    ];
    for path in lists {
        let html = page(&e, path, &admin).await;
        assert!(
            html.contains("admin-toolbar") || html.contains("admin-results"),
            "{path}: the toolbar"
        );
    }

    // Garbage in a filter is the whole list, never an error.
    for path in [
        "/admin/providers?rating=99&gap=%00&area=%00",
        "/admin/supplies?fee_min=abc&from=nope",
        "/admin/reviews?gap=stale_999",
        "/admin/partners?contract=whatever",
    ] {
        page(&e, path, &admin).await;
    }

    // A filter that is on is shown back: its box holds it, and a chip says it.
    let html = page(&e, "/admin/reviews?rating=3&reply=none", &admin).await;
    assert!(html.contains(r#"<option value="3" selected>"#));
    assert!(html.contains(r#"name="reply" value="none" checked"#));
    let html = page(&e, "/admin/supplies?fee_min=1000", &admin).await;
    assert!(html.contains(r#"name="fee_min" value="1000""#));

    // A page's own forms and links keep the toolbar's filters.
    let html = page(&e, "/admin/providers?area=Cai", &admin).await;
    assert!(html.contains(r#"value="Cai""#));
}
