//! /admin/users: the list, role changes and (de)activation, with their effect
//! on the affected user's live session. Database tests need TEST_DATABASE_URL
//! and `--include-ignored`.

use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, env_with_origin, get_page, install_admin, post_form, register_provider, send,
    sign_in, Env,
};
use sea_orm::{ConnectionTrait, Statement};

async fn user_id(e: &Env, username: &str) -> i32 {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            format!("SELECT id FROM users WHERE username = '{username}'"),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap()
}

/// An admin and the provider "nongdan", both signed in.
async fn setup(e: &Env) -> (String, String, i32) {
    let admin = install_admin(e).await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let provider = sign_in(e, "nongdan", "matkhau123").await;
    let id = user_id(e, "nongdan").await;
    (admin, provider, id)
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_list_and_detail_show_accounts_to_those_who_may_read_them() {
    let e = env_with_origin().await;
    let (admin, provider, id) = setup(&e).await;

    let list = body_text(send(&e.router, get_page("/admin/users", Some(&admin))).await).await;
    assert!(list.contains("2 tài khoản"));
    assert!(list.contains(&format!(r#"href="/admin/users/{id}""#)));

    let detail = body_text(
        send(
            &e.router,
            get_page(&format!("/admin/users/{id}"), Some(&admin)),
        )
        .await,
    )
    .await;
    assert!(detail.contains(r#"<option value="provider" selected>"#));
    assert!(
        detail.contains(r##"href="#tat-tai-khoan""##),
        "deactivate goes through a confirmation"
    );
    assert!(
        detail.contains(r#"title="own.listing.create""#),
        "effective permissions listed"
    );

    let res = send(&e.router, get_page("/admin/users", Some(&provider))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_role_change_applies_to_the_users_live_session() {
    let e = env_with_origin().await;
    let (admin, provider, id) = setup(&e).await;
    let path = format!("/admin/users/{id}/role");

    let res = send(
        &e.router,
        post_form(&path, &[("role_code", "superuser")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains("Vai trò không hợp lệ."));

    let res = send(
        &e.router,
        post_form(&path, &[("role_code", "admin")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        res.headers()[header::LOCATION],
        format!("/admin/users/{id}").as_str()
    );
    // The provider's session, untouched, now opens the admin dashboard.
    let res = send(&e.router, get_page("/admin/dashboard", Some(&provider))).await;
    assert_eq!(res.status(), StatusCode::OK);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn deactivation_signs_the_user_out_and_is_audited_with_its_reason() {
    let e = env_with_origin().await;
    let (admin, provider, id) = setup(&e).await;
    let path = format!("/admin/users/{id}/status");

    let res = send(
        &e.router,
        post_form(
            &path,
            &[("is_active", "false"), ("reason", "tài khoản giả")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let res = send(&e.router, get_page("/account", Some(&provider))).await;
    assert_eq!(
        res.headers()[header::LOCATION],
        "/login",
        "signed out at once"
    );

    let audit: String =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "SELECT action || ': ' || reason AS line FROM audit_logs \
             WHERE entity_type = 'user' AND action = 'deactivate_user'",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "line")
            .unwrap();
    assert_eq!(audit, "deactivate_user: tài khoản giả");

    let res = send(
        &e.router,
        post_form(&path, &[("is_active", "true")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    sign_in(&e, "nongdan", "matkhau123").await;
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_admin_cannot_deactivate_or_demote_themselves() {
    let e = env_with_origin().await;
    let (admin, _, _) = setup(&e).await;
    let me = user_id(&e, "quantri").await;

    let res = send(
        &e.router,
        post_form(
            &format!("/admin/users/{me}/status"),
            &[("is_active", "false")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let page = body_text(res).await;
    assert!(
        page.contains("data-open=\"true\""),
        "the confirmation re-opens with the refusal"
    );

    let res = send(
        &e.router,
        post_form(
            &format!("/admin/users/{me}/role"),
            &[("role_code", "provider")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    // Still an admin, still active.
    let res = send(&e.router, get_page("/admin/dashboard", Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::OK);
    e.cleanup().await;
}
