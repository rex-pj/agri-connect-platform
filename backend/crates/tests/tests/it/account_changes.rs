//! What an account changes about itself on /account (its name, its sign-in
//! email through the link sent to the new one, its other devices) and what an
//! admin changes for it on /admin/users/{id} (name, email the same way, a
//! password-reset link). Database tests need TEST_DATABASE_URL and
//! `--include-ignored`.

use application::constants::LOGIN_FAIL_MAX_ATTEMPTS;
use axum::http::{header, StatusCode};
use domain::ErrorCode;
use integration_tests::{
    body_text, confirm_email, cookies_from, env_with_origin, get_page, install_admin, post_form,
    register_provider, send, sign_in, Env, CLIENT,
};
use sea_orm::{ConnectionTrait, Statement};

/// An admin (so /login does not lead to /setup) and the provider "nongdan",
/// signed in; its cookies.
async fn provider(e: &Env) -> String {
    install_admin(e).await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    sign_in(e, "nongdan", "matkhau123").await
}

async fn text(e: &Env, sql: &str) -> Option<String> {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            sql.to_owned(),
        ))
        .await
        .unwrap()
        .and_then(|row| row.try_get::<Option<String>>("", "v").unwrap())
}

/// The address of the first provider account, the one each test signs in.
async fn email_of_account(e: &Env) -> String {
    text(
        e,
        "SELECT email AS v FROM users WHERE id = (SELECT min(u.id) FROM users u \n         JOIN roles r ON r.id = u.role_id WHERE r.code = 'provider')",
    )
    .await
    .unwrap()
}

async fn sign_in_fails(e: &Env, email: &str, password: &str) -> Option<ErrorCode> {
    e.state
        .usecases
        .auth
        .login(email.into(), password.into(), CLIENT)
        .await
        .err()
        .map(|err| err.code())
}

fn location(res: &axum::http::Response<axum::body::Body>) -> String {
    res.headers()[header::LOCATION].to_str().unwrap().to_owned()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_name_is_saved_cleared_and_bounded() {
    let e = env_with_origin().await;
    let me = provider(&e).await;

    let res = send(
        &e.router,
        post_form("/account/profile", &[("full_name", "  Út Hiền ")], Some(&me)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&res), "/account");
    let with_flash = format!("{me}; {}", cookies_from(&res));
    let page = body_text(send(&e.router, get_page("/account", Some(&with_flash))).await).await;
    assert!(page.contains("Đã lưu họ tên."));
    assert!(page.contains(r#"<h1 class="acct-name">Út Hiền</h1>"#));
    assert_eq!(
        text(&e, "SELECT full_name AS v FROM users WHERE email = 'nongdan@example.vn'").await,
        Some("Út Hiền".into())
    );

    let long = "Ú".repeat(201);
    let res = send(
        &e.router,
        post_form("/account/profile", &[("full_name", &long)], Some(&me)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains(r#"id="acct_name-error""#));

    // Blank clears it: the page shows the email instead.
    send(
        &e.router,
        post_form("/account/profile", &[("full_name", " ")], Some(&me)),
    )
    .await;
    assert_eq!(
        text(&e, "SELECT full_name AS v FROM users WHERE email = 'nongdan@example.vn'").await,
        None
    );
    let page = body_text(send(&e.router, get_page("/account", Some(&me))).await).await;
    assert!(page.contains(r#"<h1 class="acct-name">nongdan@example.vn</h1>"#));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_email_moves_only_once_the_link_sent_to_the_new_one_is_opened() {
    let e = env_with_origin().await;
    let me = provider(&e).await;
    // The old address was confirmed: its owner is told of the move.
    confirm_email(&e, "nongdan").await;
    // A reset link already in the old inbox, and the account's own
    // confirmation link: neither may act once the account moved.
    e.state
        .usecases
        .auth
        .forgot_password("nongdan@example.vn".into())
        .await
        .unwrap();
    let old_reset = e.mail.resets(1).await[0].1.clone();

    // The current password is asked, and a box that says nothing new is
    // refused on its field; the sheet comes back open.
    let res = send(
        &e.router,
        post_form(
            "/account/email",
            &[("new_email", "tu.moi@example.vn"), ("current_password", "sai")],
            Some(&me),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let page = body_text(res).await;
    assert!(page.contains("Mật khẩu hiện tại không đúng."));
    assert!(page.contains(r#"value="tu.moi@example.vn""#), "typed kept");
    assert!(!page.contains(r#"value="sai""#), "password never echoed");
    let res = send(
        &e.router,
        post_form(
            "/account/email",
            &[
                ("new_email", "NongDan@example.vn"),
                ("current_password", "matkhau123"),
            ],
            Some(&me),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains("Đây đã là email của tài khoản."));

    let res = send(
        &e.router,
        post_form(
            "/account/email",
            &[
                ("new_email", " Tu.Moi@Example.vn "),
                ("current_password", "matkhau123"),
            ],
            Some(&me),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&res), "/account#email");
    let sent = e.mail.email_changes(1).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "tu.moi@example.vn");
    let token = sent[0].1.clone();

    // Nothing moved yet: the page says what waits, the old address signs in.
    let page = body_text(send(&e.router, get_page("/account", Some(&me))).await).await;
    assert!(page.contains("Đang chờ xác nhận:"));
    assert!(page.contains("tu.moi@example.vn"));
    assert_eq!(email_of_account(&e).await, "nongdan@example.vn");
    assert_eq!(sign_in_fails(&e, "nongdan@example.vn", "matkhau123").await, None);

    // The link moves it, once, confirmed.
    let form = [("token", token.as_str())];
    let res = send(&e.router, post_form("/verify-email", &form, None)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let page = body_text(res).await;
    assert!(page.contains("Đã đổi email"));
    assert!(page.contains("tu.moi@example.vn"));
    assert_eq!(email_of_account(&e).await, "tu.moi@example.vn");
    assert!(
        text(&e, "SELECT email_verified_at::text AS v FROM users WHERE email = 'tu.moi@example.vn'")
            .await
            .is_some(),
        "the new address is confirmed"
    );
    let res = send(&e.router, post_form("/verify-email", &form, None)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "spent");

    // Signed in by the new address only; the old one was told, masked.
    assert_eq!(sign_in_fails(&e, "tu.moi@example.vn", "matkhau123").await, None);
    assert_eq!(
        sign_in_fails(&e, "nongdan@example.vn", "matkhau123").await,
        Some(ErrorCode::InvalidCredentials)
    );
    let told = e.mail.email_changed(1).await;
    assert_eq!(
        told,
        [("nongdan@example.vn".to_owned(), "t*****@example.vn".to_owned())]
    );
    // The reset link sent to the old address no longer works.
    assert_eq!(
        e.state
            .usecases
            .auth
            .reset_password(old_reset, "matkhaumoi123".into())
            .await
            .err()
            .map(|err| err.code()),
        Some(ErrorCode::ResetTokenInvalid)
    );
    // Audited on the account, in words on its admin page.
    assert_eq!(
        text(
            &e,
            "SELECT string_agg(action, ',' ORDER BY id) AS v FROM audit_logs \
             WHERE action IN ('request_email_change', 'change_email')"
        )
        .await,
        Some("request_email_change,change_email".into())
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_address_another_account_has_is_never_said_nor_taken() {
    let e = env_with_origin().await;
    let me = provider(&e).await;
    register_provider(&e.state, "hangxom", "0907654321").await;

    // The same answer as for a free address, and nothing is sent.
    let res = send(
        &e.router,
        post_form(
            "/account/email",
            &[
                ("new_email", "hangxom@example.vn"),
                ("current_password", "matkhau123"),
            ],
            Some(&me),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(e.mail.email_changes(1).await.is_empty());

    // A free address asked for, then taken by a sign-up before the link is
    // opened: the link is refused and the account keeps its address.
    send(
        &e.router,
        post_form(
            "/account/email",
            &[
                ("new_email", "chuacoai@example.vn"),
                ("current_password", "matkhau123"),
            ],
            Some(&me),
        ),
    )
    .await;
    let token = e.mail.email_changes(1).await[0].1.clone();
    register_provider(&e.state, "chuacoai", "0912345678").await;
    let res = send(
        &e.router,
        post_form("/verify-email", &[("token", token.as_str())], None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert!(body_text(res).await.contains("Email này đã có tài khoản khác"));
    assert_eq!(sign_in_fails(&e, "nongdan@example.vn", "matkhau123").await, None);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn every_form_that_asks_the_password_counts_the_wrong_ones_together() {
    let e = env_with_origin().await;
    provider(&e).await;
    let auth = &e.state.usecases.auth;
    let user_id = auth
        .login("nongdan@example.vn".into(), "matkhau123".into(), CLIENT)
        .await
        .unwrap()
        .user_id;
    for i in 0..LOGIN_FAIL_MAX_ATTEMPTS {
        let code = if i % 2 == 0 {
            auth.request_email_change(user_id, "sai".into(), "moi@example.vn".into())
                .await
        } else {
            auth.change_password(user_id, "sai".into(), "matkhaumoi123".into())
                .await
        }
        .err()
        .map(|err| err.code());
        assert_eq!(code, Some(ErrorCode::CurrentPasswordIncorrect));
    }
    // Now even the right one is not checked, on either form.
    assert_eq!(
        auth.request_email_change(user_id, "matkhau123".into(), "moi@example.vn".into())
            .await
            .err()
            .map(|err| err.code()),
        Some(ErrorCode::TooManyRequests)
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_other_devices_are_signed_out_and_this_one_stays() {
    let e = env_with_origin().await;
    let phone = provider(&e).await;
    let laptop = sign_in(&e, "nongdan", "matkhau123").await;

    let res = send(
        &e.router,
        post_form("/account/sessions", &[], Some(&laptop)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&res), "/account#security");
    let laptop = cookies_from(&res);
    let page = body_text(send(&e.router, get_page("/account", Some(&laptop))).await).await;
    assert!(page.contains("Đã đăng xuất mọi thiết bị khác."));

    // The phone's session ended: its refresh token renews nothing.
    let phone_refresh_only: String = phone
        .split("; ")
        .filter(|c| c.starts_with("refresh_token="))
        .collect();
    let res = send(&e.router, get_page("/account", Some(&phone_refresh_only))).await;
    assert_eq!(location(&res), "/login");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_admin_corrects_an_account_without_ever_holding_its_sign_in() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let id = text(&e, "SELECT id::text AS v FROM users WHERE email = 'nongdan@example.vn'")
        .await
        .unwrap();
    let me = text(&e, "SELECT id::text AS v FROM users WHERE email = 'quantri@example.vn'")
        .await
        .unwrap();
    let page = format!("/admin/users/{id}");
    let profile = format!("{page}/profile");

    // Both boxes are checked before either is saved.
    let res = send(
        &e.router,
        post_form(
            &profile,
            &[("full_name", "Út Hiền"), ("new_email", "khong-phai-email")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains(r#"id="eu_email-error""#));
    assert_eq!(
        text(&e, "SELECT full_name AS v FROM users WHERE email = 'nongdan@example.vn'").await,
        None
    );

    // The name is saved; the address only waits for its link.
    let res = send(
        &e.router,
        post_form(
            &profile,
            &[
                ("full_name", "Út Hiền"),
                ("new_email", "uthien@example.vn"),
                ("reason", "gõ nhầm lúc đăng ký"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(e.mail.email_changes(1).await[0].0, "uthien@example.vn");
    assert_eq!(email_of_account(&e).await, "nongdan@example.vn");
    let detail = body_text(send(&e.router, get_page(&page, Some(&admin))).await).await;
    assert!(detail.contains("Út Hiền"));
    assert!(detail.contains("Đang chờ xác nhận:"));
    assert_eq!(
        text(
            &e,
            "SELECT string_agg(action || ': ' || reason, ',' ORDER BY id) AS v FROM audit_logs \
             WHERE action IN ('update_profile', 'request_email_change')"
        )
        .await,
        Some(
            "update_profile: gõ nhầm lúc đăng ký,request_email_change: gõ nhầm lúc đăng ký".into()
        )
    );

    // A reset link goes to the account's own address; the admin sees no
    // password at any point.
    let res = send(
        &e.router,
        post_form(&format!("{page}/password-reset"), &[], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(e.mail.resets(1).await[0].0, "nongdan@example.vn");

    // Never on the admin's own account (the account page asks the
    // password), and never for an account without the permission.
    let own = body_text(
        send(&e.router, get_page(&format!("/admin/users/{me}"), Some(&admin))).await,
    )
    .await;
    assert!(!own.contains(r#"id="sua-tai-khoan""#));
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/users/{me}/profile"),
            &[("full_name", "Khac"), ("new_email", "khac@example.vn")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    let res = send(
        &e.router,
        post_form(&profile, &[("full_name", "Toi")], Some(&provider)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}
