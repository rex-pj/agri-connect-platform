//! /account: the profile, password change (which signs out other devices)
//! and logout. Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, cookies_from, env_with_origin, get_page, install_admin, post_form,
    register_provider, send, sign_in, Env,
};

/// An admin (so /login does not redirect to /setup) and the provider "nongdan".
async fn installed(e: &Env) {
    install_admin(e).await;
    register_provider(&e.state, "nongdan", "0901234567").await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_account_page_needs_a_session_and_shows_the_profile() {
    let e = env_with_origin().await;
    installed(&e).await;
    let res = send(&e.router, get_page("/account", None)).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/login");

    let cookies = sign_in(&e, "nongdan", "matkhau123").await;
    let res = send(&e.router, get_page("/account", Some(&cookies))).await;
    assert_eq!(res.status(), StatusCode::OK);
    let page = body_text(res).await;
    assert!(page.contains(r#"<meta name="robots" content="noindex, nofollow">"#));
    assert!(page.contains("nongdan@example.vn"));
    assert!(page.contains("Nhà cung cấp"));
    assert!(
        page.contains(r#"href="/provider/dashboard""#),
        "link to their area"
    );
    assert!(page.contains(r#"action="/logout""#));
    assert!(page.contains(r#"id="security""#));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_password_change_keeps_this_device_and_signs_out_the_others() {
    let e = env_with_origin().await;
    installed(&e).await;
    let phone = sign_in(&e, "nongdan", "matkhau123").await;
    let laptop = sign_in(&e, "nongdan", "matkhau123").await;

    let wrong = [
        ("current_password", "sai"),
        ("new_password", "matkhaumoi1"),
        ("password2", "matkhaumoi1"),
    ];
    let res = send(
        &e.router,
        post_form("/account/password", &wrong, Some(&laptop)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert!(body_text(res)
        .await
        .contains("Mật khẩu hiện tại không đúng."));

    let mismatch = [
        ("current_password", "matkhau123"),
        ("new_password", "matkhaumoi1"),
        ("password2", "matkhaumoi2"),
    ];
    let res = send(
        &e.router,
        post_form("/account/password", &mismatch, Some(&laptop)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res)
        .await
        .contains("Mật khẩu xác nhận không khớp."));

    let good = [
        ("current_password", "matkhau123"),
        ("new_password", "matkhaumoi1"),
        ("password2", "matkhaumoi1"),
    ];
    let res = send(
        &e.router,
        post_form("/account/password", &good, Some(&laptop)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/account#security");
    let laptop = cookies_from(&res);
    assert!(
        laptop.contains("refresh_token="),
        "this device gets a fresh session"
    );

    let page = body_text(send(&e.router, get_page("/account", Some(&laptop))).await).await;
    assert!(page.contains("Đã đổi mật khẩu."));

    // The phone's refresh token was revoked: without a live access token it
    // is signed out.
    let phone_refresh_only: String = phone
        .split("; ")
        .filter(|c| c.starts_with("refresh_token="))
        .collect();
    let res = send(&e.router, get_page("/account", Some(&phone_refresh_only))).await;
    assert_eq!(res.headers()[header::LOCATION], "/login");

    sign_in(&e, "nongdan", "matkhaumoi1").await;
    e.cleanup().await;
}

/// The bottom tab that is lit on `page` (wide or phone), by its link.
fn lit_tab(page: &str) -> Option<&str> {
    page.split("<a class=\"bottom-nav__tab")
        .skip(1)
        .find(|tab| {
            tab.starts_with(" bottom-nav__tab--active")
                && !tab.starts_with(" bottom-nav__tab--active-phone")
        })
        .and_then(|tab| tab.split("href=\"").nth(1))
        .and_then(|rest| rest.split('"').next())
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_admin_keeps_the_panel_on_the_profile_and_on_an_error() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    let page = body_text(send(&e.router, get_page("/account", Some(&admin))).await).await;
    assert!(page.contains("ash-sidebar"), "the admin frame");
    assert!(!page.contains("bottom-nav"), "not the farmers' tabs");
    assert!(page.contains(r#"action="/admin/logout""#));
    assert!(page.contains(r#"id="security""#));

    let res = send(&e.router, get_page("/admin/users/999999", Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let page = body_text(res).await;
    assert!(
        page.contains("ash-sidebar"),
        "a missing record keeps the panel"
    );
    assert!(
        page.contains(r#"class="btn-pill" href="/admin/dashboard""#),
        "the way back is the panel"
    );

    // Outside the panel, an error is an app page, still signed in.
    let page = body_text(send(&e.router, get_page("/supplies/999999/x", Some(&admin))).await).await;
    assert!(page.contains("bottom-nav"));
    assert!(
        page.contains(r#"action="/logout""#),
        "the account menu, signed in"
    );
    assert!(page.contains(r#"class="btn-pill" href="/""#));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_provider_area_keeps_its_tab_lit_and_its_way_back() {
    let e = env_with_origin().await;
    installed(&e).await;
    let provider = sign_in(&e, "nongdan", "matkhau123").await;

    for path in [
        "/provider/dashboard",
        "/provider/reviews",
        "/provider/supplies/new",
    ] {
        let page = body_text(send(&e.router, get_page(path, Some(&provider))).await).await;
        assert_eq!(lit_tab(&page), Some("/provider/dashboard"), "{path}");
    }

    let res = send(
        &e.router,
        get_page("/provider/services/999999/edit", Some(&provider)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let page = body_text(res).await;
    assert!(
        page.contains(r#"class="btn-pill" href="/provider/dashboard""#),
        "back to the dashboard, not home"
    );
    assert_eq!(lit_tab(&page), Some("/provider/dashboard"));
    e.cleanup().await;
}
