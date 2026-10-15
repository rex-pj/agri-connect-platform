//! /account: the profile, password change (which signs out other devices)
//! and logout. Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, cookies_from, env_with_origin, get_page, post_form, register_provider, send, Env,
};

/// Signs in through the login page and returns the cookies.
async fn sign_in(e: &Env, password: &str) -> String {
    let res = send(
        &e.router,
        post_form(
            "/login",
            &[("username", "nongdan"), ("password", password)],
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "login with {password}");
    cookies_from(&res)
}

/// Sets the first admin up, so /login does not redirect to /setup.
async fn installed(e: &Env) {
    send(
        &e.router,
        post_form(
            "/setup",
            &[
                ("email", "admin@example.vn"),
                ("username", "quantri"),
                ("password", "matkhau123"),
                ("password2", "matkhau123"),
            ],
            None,
        ),
    )
    .await;
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

    let cookies = sign_in(&e, "matkhau123").await;
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
    let phone = sign_in(&e, "matkhau123").await;
    let laptop = sign_in(&e, "matkhau123").await;

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

    sign_in(&e, "matkhaumoi1").await;
    e.cleanup().await;
}
