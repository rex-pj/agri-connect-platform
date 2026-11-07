//! /admin/explore: the provider list, creating and editing a provider,
//! verification and account assignment.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, env_with_origin, get_page, install_admin, post_form, register_provider, send,
    sign_in, Env,
};
use sea_orm::{ConnectionTrait, Statement};

async fn scalar(e: &Env, sql: &str, column: &str) -> String {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            sql.to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", column)
        .unwrap()
}

/// A business entered by an admin, with no account yet.
async fn unclaimed_business(e: &Env) -> i32 {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            "INSERT INTO businesses (name, phone) VALUES ('Máy cày Út Hiền', '0912345678') \
             RETURNING id"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn verification_is_an_explicit_state_and_withdrawal_is_audited() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let id = unclaimed_business(&e).await;
    let detail = format!("/admin/explore/{id}");
    let action = format!("{detail}/verification");

    let list = body_text(send(&e.router, get_page("/admin/explore", Some(&admin))).await).await;
    assert!(list.contains(&format!(r#"href="{detail}""#)));

    // Sent twice: still verified, audited once.
    for _ in 0..2 {
        let res = send(
            &e.router,
            post_form(&action, &[("is_verified", "true")], Some(&admin)),
        )
        .await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(res.headers()[header::LOCATION], detail.as_str());
    }
    let audited = scalar(
        &e,
        "SELECT count(*)::text AS n FROM audit_logs WHERE action = 'verify_provider'",
        "n",
    )
    .await;
    assert_eq!(audited, "1");
    let page = body_text(send(&e.router, get_page(&detail, Some(&admin))).await).await;
    assert!(
        page.contains(r##"href="#bo-xac-minh""##),
        "withdrawal is confirmed"
    );

    let res = send(
        &e.router,
        post_form(
            &action,
            &[("is_verified", "false"), ("reason", "số điện thoại sai")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let line = scalar(
        &e,
        "SELECT action || ': ' || reason AS line FROM audit_logs \
         WHERE action = 'unverify_provider'",
        "line",
    )
    .await;
    assert_eq!(line, "unverify_provider: số điện thoại sai");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn turning_a_provider_off_hides_it_from_the_public_and_is_audited() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let id = unclaimed_business(&e).await;
    let detail = format!("/admin/explore/{id}");
    let action = format!("{detail}/status");
    let public = || async { body_text(send(&e.router, get_page("/providers", None)).await).await };
    assert!(public().await.contains("Máy cày Út Hiền"));

    let page = body_text(send(&e.router, get_page(&detail, Some(&admin))).await).await;
    assert!(
        page.contains(r##"href="#tat-nha-cung-cap""##),
        "turning off is confirmed"
    );
    let res = send(
        &e.router,
        post_form(
            &action,
            &[("is_active", "false"), ("reason", "ngừng hoạt động")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], detail.as_str());
    assert_eq!(
        scalar(
            &e,
            "SELECT action || ': ' || reason AS line FROM audit_logs \
             WHERE action = 'deactivate_provider'",
            "line",
        )
        .await,
        "deactivate_provider: ngừng hoạt động"
    );
    assert!(
        !public().await.contains("Máy cày Út Hiền"),
        "gone from the public"
    );
    let page = body_text(send(&e.router, get_page(&detail, Some(&admin))).await).await;
    assert!(page.contains("Bật lại nhà cung cấp"));

    let res = send(
        &e.router,
        post_form(&action, &[("is_active", "true")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(public().await.contains("Máy cày Út Hiền"), "listed again");
    assert_eq!(
        scalar(
            &e,
            "SELECT count(*)::text AS n FROM audit_logs WHERE action = 'activate_provider'",
            "n",
        )
        .await,
        "1"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_new_account_needs_a_valid_email_and_password_and_can_then_sign_in() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let id = unclaimed_business(&e).await;
    let action = format!("/admin/explore/{id}/account");

    // The form starts with the business phone as the username.
    let page = body_text(
        send(
            &e.router,
            get_page(&format!("/admin/explore/{id}"), Some(&admin)),
        )
        .await,
    )
    .await;
    assert!(page.contains(r#"value="0912345678""#));

    let res = send(
        &e.router,
        post_form(
            &action,
            &[
                ("username", "0912345678"),
                ("email", "uthien"),
                ("password", "ngan"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Email không hợp lệ"));
    assert!(html.contains("Mật khẩu phải có ít nhất"));
    assert!(html.contains(r#"value="uthien""#), "typed email kept");
    assert!(!html.contains(r#"value="ngan""#), "password never echoed");

    // An email another account uses is refused on its field, not a 500.
    let res = send(
        &e.router,
        post_form(
            &action,
            &[
                ("username", "0912345678"),
                ("email", "admin@example.vn"),
                ("password", "matkhau123"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains(r#"id="acc-email-error""#));

    let res = send(
        &e.router,
        post_form(
            &action,
            &[
                ("username", "0912345678"),
                ("email", "uthien@example.vn"),
                ("password", "matkhau123"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let line = scalar(
        &e,
        "SELECT action || ': ' || reason AS line FROM audit_logs \
         WHERE action = 'assign_provider_account'",
        "line",
    )
    .await;
    assert_eq!(line, "assign_provider_account: 0912345678");
    sign_in(&e, "0912345678", "matkhau123").await;
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_account_manages_one_business_and_admins_cannot_be_linked() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let id = unclaimed_business(&e).await;
    let action = format!("/admin/explore/{id}/account");

    // "nongdan" already manages the business it registered.
    let res = send(
        &e.router,
        post_form(&action, &[("username", "nongdan")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Tài khoản này đang quản lý một nhà cung cấp khác."));
    assert!(
        html.contains(r#"data-open="true""#),
        "the account sheet comes back open"
    );

    let res = send(
        &e.router,
        post_form(&action, &[("username", "quantri")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // A provider without the assign permission cannot use the page at all.
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    let res = send(
        &e.router,
        get_page(&format!("/admin/explore/{id}"), Some(&provider)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_json_api_sets_verification_explicitly() {
    let e = env_with_origin().await;
    install_admin(&e).await;
    let token = e
        .state
        .usecases
        .auth
        .login("quantri".into(), "matkhau123".into())
        .await
        .unwrap()
        .token;
    let id = unclaimed_business(&e).await;
    let req = axum::http::Request::patch(format!("/api/admin/providers/{id}/verification"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(r#"{"is_verified":true}"#))
        .unwrap();
    let body = body_text(send(&e.router, req).await).await;
    assert!(body.contains(r#""is_verified":true"#), "{body}");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_admin_lists_a_provider_in_the_field_and_edits_its_profile() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let list = body_text(send(&e.router, get_page("/admin/explore", Some(&admin))).await).await;
    assert!(
        list.contains(r##"href="#them-nha-cung-cap""##),
        "the create sheet is offered"
    );

    // A broken rule: the sheet comes back open with the values as typed.
    let res = send(
        &e.router,
        post_form(
            "/admin/explore",
            &[("name", "Drone Ba Tu"), ("phone", "12345")],
            Some(&admin),
        ),
    )
    .await;
    assert!(res.status().is_client_error());
    let html = body_text(res).await;
    assert!(html.contains(r#"data-open="true""#));
    assert!(html.contains(r#"value="Drone Ba Tu""#));

    let res = send(
        &e.router,
        post_form(
            "/admin/explore",
            &[
                ("name", "Drone Ba Tu"),
                ("phone", "0912345678"),
                ("address", "Cai Lay, Tien Giang"),
                ("lat", "10.4048"),
                ("lng", "106.118"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let detail = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    let id = scalar(&e, "SELECT id::text AS id FROM businesses", "id").await;
    assert_eq!(detail, format!("/admin/explore/{id}"));
    assert_eq!(
        scalar(
            &e,
            "SELECT round(ST_Y(location::geometry)::numeric, 4)::text AS lat FROM businesses",
            "lat",
        )
        .await,
        "10.4048"
    );
    let public = body_text(send(&e.router, get_page("/providers", None)).await).await;
    assert!(public.contains("Drone Ba Tu"), "listed with no account");

    let res = send(
        &e.router,
        post_form(
            &detail,
            &[("name", "Drone Ba Tu Cai Lay"), ("phone", "0912345678")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        scalar(
            &e,
            "SELECT name || ' ' || (location IS NULL)::text AS line FROM businesses",
            "line",
        )
        .await,
        "Drone Ba Tu Cai Lay true",
        "renamed, and the blank coordinates took the pin off"
    );
    assert_eq!(
        scalar(
            &e,
            "SELECT string_agg(action, ',' ORDER BY id) AS a FROM audit_logs \
             WHERE action LIKE '%_provider'",
            "a",
        )
        .await,
        "create_provider,update_provider"
    );

    // Someone else's profile form is the owner's dashboard, never this page.
    register_provider(&e.state, "nongdan", "0901234567").await;
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    let res = send(
        &e.router,
        post_form(
            "/admin/explore",
            &[("name", "X"), ("phone", "0912345678")],
            Some(&provider),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}
