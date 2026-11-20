//! The Rust auth pages: first-run setup, login, registration, password reset
//! and the demo data page, through the real router with real cookies.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, cookies_from, env_with_origin, get_page, known_reset_token, post_form,
    register_provider, send, Env,
};
use sea_orm::{ConnectionTrait, Statement};

fn location(res: &axum::http::Response<axum::body::Body>) -> String {
    res.headers()[header::LOCATION].to_str().unwrap().to_owned()
}

async fn count(e: &Env, sql: &str) -> i64 {
    e.t.db
        .query_one(Statement::from_string(e.t.db.get_database_backend(), sql))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "n")
        .unwrap()
}

const ADMIN: &[(&str, &str)] = &[
    ("full_name", "Quản Trị"),
    ("email", "admin@example.vn"),
    ("username", "quantri"),
    ("password", "matkhau123"),
    ("password2", "matkhau123"),
];

/// Runs the setup form and returns the session cookies it set.
async fn set_up(e: &Env, extra: &[(&str, &str)]) -> String {
    let mut form = ADMIN.to_vec();
    form.extend_from_slice(extra);
    let res = send(&e.router, post_form("/setup", &form, None)).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        location(&res),
        "/admin/dashboard",
        "an admin lands on the admin area"
    );
    cookies_from(&res)
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn with_a_setup_token_the_first_admin_needs_it() {
    const TOKEN: &str = "ma-thiet-lap-rieng-cua-may-chu";
    let e = Env::new().await;
    // No SETUP_TOKEN: the form does not ask for one.
    let open = body_text(send(&e.router, get_page("/setup", None)).await).await;
    assert!(!open.contains(r#"name="setup_token""#));

    let router = integration_tests::router_with(
        integration_tests::state_with(
            e.t.db.clone(),
            integration_tests::memory_uploads(None),
            integration_tests::email_service(),
            Some(TOKEN.to_owned()),
        ),
        web::routings::app_routing::RouterConfig {
            allowed_origins: vec![integration_tests::TEST_ORIGIN.to_owned()],
            ..Default::default()
        },
    );
    let page = body_text(send(&router, get_page("/setup", None)).await).await;
    assert!(page.contains(r#"name="setup_token""#));

    for wrong in [None, Some("doan-mo")] {
        let mut form = ADMIN.to_vec();
        form.extend(wrong.map(|t| ("setup_token", t)));
        let res = send(&router, post_form("/setup", &form, None)).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{wrong:?}");
        let page = body_text(res).await;
        assert!(
            page.contains(r#"id="su_token-error""#),
            "the form comes back with the token box at fault"
        );
        assert!(!page.contains("doan-mo"), "the token is never echoed back");
    }
    assert_eq!(
        count(&e, "SELECT count(*) AS n FROM users").await,
        0,
        "no account without the token"
    );

    let mut form = ADMIN.to_vec();
    form.push(("setup_token", TOKEN));
    let res = send(&router, post_form("/setup", &form, None)).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&res), "/admin/dashboard");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_fresh_install_goes_to_setup_and_setup_closes_after_use() {
    let e = env_with_origin().await;
    // Bug 0.2: with no admin yet, /login leads to /setup.
    let res = send(&e.router, get_page("/login", None)).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&res), "/setup");

    let page = body_text(send(&e.router, get_page("/setup", None)).await).await;
    assert!(page.contains(r#"<meta name="robots" content="noindex, nofollow">"#));
    assert!(
        page.contains(r#"name="demo">"#),
        "demo data is opt-in: unticked"
    );

    let cookies = set_up(&e, &[]).await;
    assert!(cookies.contains("auth_token=") && cookies.contains("refresh_token="));
    assert_eq!(
        count(&e, "SELECT count(*) AS n FROM businesses WHERE is_demo").await,
        0
    );

    // Once there is an admin, /setup is closed and /login is the way in.
    let res = send(&e.router, get_page("/setup", None)).await;
    assert_eq!(location(&res), "/login");
    // Bug 0.1: a signed-in visitor on /login goes to their own area.
    let res = send(&e.router, get_page("/login", Some(&cookies))).await;
    assert_eq!(location(&res), "/admin/dashboard");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn setup_can_load_the_demo_data_and_rejects_a_mistyped_password() {
    let e = env_with_origin().await;
    let mut bad = ADMIN.to_vec();
    bad.retain(|(k, _)| *k != "password2");
    bad.push(("password2", "matkhau12"));
    let res = send(&e.router, post_form("/setup", &bad, None)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let page = body_text(res).await;
    assert!(page.contains("Mật khẩu xác nhận không khớp."));
    assert!(page.contains(r#"value="quantri""#), "username kept");
    assert!(!page.contains("matkhau"), "passwords are never echoed");
    assert_eq!(count(&e, "SELECT count(*) AS n FROM users").await, 0);

    let cookies = set_up(&e, &[("demo", "on")]).await;
    assert!(count(&e, "SELECT count(*) AS n FROM businesses WHERE is_demo").await > 0);
    assert!(cookies.contains("flash=demo_loaded"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn login_lands_by_permission_and_locks_after_repeated_failures() {
    let e = env_with_origin().await;
    set_up(&e, &[]).await;
    register_provider(&e.state, "nongdan", "0901234567").await;

    let res = send(
        &e.router,
        post_form(
            "/login",
            &[("username", "nongdan"), ("password", "sai-mat-khau")],
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let page = body_text(res).await;
    assert!(page.contains("tên đăng nhập hoặc mật khẩu."));
    assert!(page.contains(r#"value="nongdan""#));

    // The box takes the email too.
    let res = send(
        &e.router,
        post_form(
            "/login",
            &[
                ("username", "NongDan@example.vn"),
                ("password", "matkhau123"),
            ],
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&res), "/provider/dashboard");

    let res = send(
        &e.router,
        post_form(
            "/login",
            &[("username", "nongdan"), ("password", "matkhau123")],
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        location(&res),
        "/provider/dashboard",
        "a provider lands on the dashboard"
    );
    assert!(cookies_from(&res).contains("refresh_token="));

    for _ in 0..5 {
        send(
            &e.router,
            post_form(
                "/login",
                &[("username", "nongdan"), ("password", "sai")],
                None,
            ),
        )
        .await;
    }
    let res = send(
        &e.router,
        post_form(
            "/login",
            &[("username", "nongdan"), ("password", "matkhau123")],
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::LOCKED);
    let page = body_text(res).await;
    assert!(page.contains("Tài khoản tạm thời bị khoá"));
    assert!(page.contains(r#"href="/forgot-password""#));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn registration_signs_the_provider_in_or_says_what_to_fix() {
    let e = env_with_origin().await;
    let good = [
        ("business_name", "Vườn Út Hiền"),
        ("business_phone", "0901234567"),
        ("full_name", "Út Hiền"),
        ("email", "uthien@example.vn"),
        ("username", "uthien"),
        ("password", "matkhau123"),
        ("password2", "matkhau123"),
    ];
    let mut bad = good.to_vec();
    bad[1] = ("business_phone", "12345");
    bad[3] = ("email", "uthien@gmail");
    let res = send(&e.router, post_form("/register", &bad, None)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let page = body_text(res).await;
    assert!(
        page.contains(r#"id="r_biz_phone-error""#),
        "phone message under the phone field"
    );
    assert!(
        page.contains("Email không hợp lệ"),
        "email checked in the same round trip"
    );
    assert!(
        page.contains(r#"value="Vườn Út Hiền""#),
        "business name kept"
    );

    // Too long for the columns: field messages, not a server error.
    let long_business = "V".repeat(201);
    let long_name = "Ú".repeat(201);
    let long_user = "u".repeat(51);
    let mut long = good.to_vec();
    long[0] = ("business_name", &long_business);
    long[2] = ("full_name", &long_name);
    long[4] = ("username", &long_user);
    let res = send(&e.router, post_form("/register", &long, None)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let page = body_text(res).await;
    for field in ["r_biz_name", "r_fullname", "r_user"] {
        assert!(page.contains(&format!(r#"id="{field}-error""#)), "{field}");
    }
    let mut spaced = good.to_vec();
    spaced[4] = ("username", "ut hien");
    let page = body_text(send(&e.router, post_form("/register", &spaced, None)).await).await;
    assert!(page.contains("Tên đăng nhập không được có khoảng trắng."));
    assert_eq!(count(&e, "SELECT count(*) AS n FROM users").await, 0);

    let res = send(&e.router, post_form("/register", &good, None)).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&res), "/provider/dashboard");
    let cookies = cookies_from(&res);

    // The account and its business were made together.
    assert_eq!(
        count(
            &e,
            "SELECT count(*) AS n FROM businesses b JOIN users u ON u.id = b.owner_id \
             WHERE u.username = 'uthien'"
        )
        .await,
        1
    );

    // Signed in, /register and /login point to the dashboard.
    for page in ["/register", "/login"] {
        let res = send(&e.router, get_page(page, Some(&cookies))).await;
        assert_eq!(location(&res), "/provider/dashboard", "{page}");
    }
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_password_reset_flow_ends_on_the_login_page_once() {
    let e = env_with_origin().await;
    set_up(&e, &[]).await;
    register_provider(&e.state, "nongdan", "0901234567").await;

    for email in ["nongdan@example.vn", "khong-ai@example.vn"] {
        let res = send(
            &e.router,
            post_form("/forgot-password", &[("email", email)], None),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            body_text(res).await.contains("Kiểm tra hộp thư"),
            "same answer for {email}"
        );
    }
    // Only one email existed, so only one token was made; it is stored hashed.
    let stored: String =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "SELECT token FROM password_reset_tokens",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "token")
            .unwrap();
    assert_eq!(stored.len(), 64, "hex SHA-256, never the emailed token");
    let token = known_reset_token(&e, "lien-ket-trong-email").await;

    let page = body_text(send(&e.router, get_page("/reset-password", None)).await).await;
    assert!(page.contains("Liên kết không hợp lệ"), "no token");

    let form = [
        ("token", token.as_str()),
        ("new_password", "matkhaumoi1"),
        ("password2", "matkhaumoi1"),
    ];
    let res = send(&e.router, post_form("/reset-password", &form, None)).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&res), "/login");
    let flash = cookies_from(&res);
    let page = body_text(send(&e.router, get_page("/login", Some(&flash))).await).await;
    assert!(page.contains("Mật khẩu đã được đặt lại."));

    let res = send(&e.router, post_form("/reset-password", &form, None)).await;
    assert!(
        body_text(res).await.contains("Liên kết không hợp lệ"),
        "a token works once"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_demo_data_page_is_for_admins_and_removal_needs_a_tick() {
    let e = env_with_origin().await;
    let admin = set_up(&e, &[("demo", "on")]).await;
    let res = send(&e.router, get_page("/setup/seed", None)).await;
    assert_eq!(location(&res), "/login", "anonymous");

    let provider = {
        register_provider(&e.state, "nongdan", "0901234567").await;
        let res = send(
            &e.router,
            post_form(
                "/login",
                &[("username", "nongdan"), ("password", "matkhau123")],
                None,
            ),
        )
        .await;
        cookies_from(&res)
    };
    let res = send(&e.router, get_page("/setup/seed", Some(&provider))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let page = body_text(send(&e.router, get_page("/setup/seed", Some(&admin))).await).await;
    assert!(page.contains("Nhà cung cấp mẫu"));

    let res = send(
        &e.router,
        post_form("/setup/seed", &[("action", "remove")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains("Vui lòng đánh dấu xác nhận"));
    assert!(count(&e, "SELECT count(*) AS n FROM businesses WHERE is_demo").await > 0);

    let res = send(
        &e.router,
        post_form(
            "/setup/seed",
            &[("action", "remove"), ("confirm", "on")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        count(&e, "SELECT count(*) AS n FROM businesses WHERE is_demo").await,
        0
    );
    assert_eq!(
        count(&e, "SELECT count(*) AS n FROM businesses WHERE NOT is_demo").await,
        1,
        "the real provider's business survives"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_login_form_posted_from_another_site_is_refused() {
    let e = env_with_origin().await;
    set_up(&e, &[]).await;
    let mut req = post_form(
        "/login",
        &[("username", "quantri"), ("password", "matkhau123")],
        None,
    );
    req.headers_mut()
        .insert(header::ORIGIN, "https://evil.example".parse().unwrap());
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert!(cookies_from(&res).is_empty());
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_sign_up_confirms_its_address_with_the_emailed_link() {
    let e = env_with_origin().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let confirmed = "SELECT count(*) AS n FROM users WHERE email_verified_at IS NOT NULL";

    let sent = e.mail.verifications(1).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "nongdan@example.vn");
    let token = sent[0].1.clone();
    assert_eq!(count(&e, confirmed).await, 0);

    // The account page says so and offers the link again.
    let cookie = integration_tests::sign_in(&e, "nongdan", "matkhau123").await;
    let account = body_text(send(&e.router, get_page("/account", Some(&cookie))).await).await;
    assert!(account.contains(r#"action="/account/email-verification""#));
    let res = send(
        &e.router,
        post_form("/account/email-verification", &[], Some(&cookie)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&res), "/account");
    assert_eq!(e.mail.verifications(2).await.len(), 2);

    // Opening the link (a mail scanner does too) confirms nothing.
    let link = format!("/verify-email?token={token}");
    let page = body_text(send(&e.router, get_page(&link, None)).await).await;
    assert!(page.contains(r#"name="token""#));
    assert_eq!(count(&e, confirmed).await, 0);

    // A wrong token is refused; the form confirms, once.
    let res = send(
        &e.router,
        post_form("/verify-email", &[("token", "khong-dung")], None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let form = [("token", token.as_str())];
    let res = send(&e.router, post_form("/verify-email", &form, None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(count(&e, confirmed).await, 1);
    let res = send(&e.router, post_form("/verify-email", &form, None)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let account = body_text(send(&e.router, get_page("/account", Some(&cookie))).await).await;
    assert!(!account.contains(r#"action="/account/email-verification""#));
    e.cleanup().await;
}
