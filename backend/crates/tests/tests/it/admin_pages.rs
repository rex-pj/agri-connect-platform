//! The Rust admin frame, dashboard, roles and the permission grid, end to end:
//! a grant made on the grid opens a page for that role on the next request.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, env_with_origin, get_page, install_admin, post_form, register_provider, send,
    sign_in, Env,
};
use sea_orm::{ConnectionTrait, Statement};

async fn role_id(e: &Env, code: &str) -> i32 {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            format!("SELECT id FROM roles WHERE code = '{code}'"),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap()
}

/// The permission keys `code` grants now, as `perm.<key>` form fields.
async fn current_grid(e: &Env, code: &str) -> Vec<String> {
    e.t.db
        .query_all(Statement::from_string(
            e.t.db.get_database_backend(),
            format!(
                "SELECT p.code FROM role_permissions rp
                 JOIN permissions p ON p.id = rp.permission_id
                 JOIN roles r ON r.id = rp.role_id
                 WHERE r.code = '{code}' ORDER BY p.code"
            ),
        ))
        .await
        .unwrap()
        .iter()
        .map(|r| format!("perm.{}", r.try_get::<String>("", "code").unwrap()))
        .collect()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_dashboard_and_its_menu_follow_permissions() {
    let e = env_with_origin().await;
    let res = send(&e.router, get_page("/admin", None)).await;
    assert_eq!(res.headers()[header::LOCATION], "/login");

    let admin = install_admin(&e).await;
    let res = send(&e.router, get_page("/admin", Some(&admin))).await;
    assert_eq!(res.headers()[header::LOCATION], "/admin/dashboard");

    let page = body_text(send(&e.router, get_page("/admin/dashboard", Some(&admin))).await).await;
    assert!(page.contains(r#"<meta name="robots" content="noindex, nofollow">"#));
    assert!(page.contains("Tổng nhà cung cấp"));
    for item in ["Nhà cung cấp", "Vai trò", "Quyền hạn", "Dữ liệu mẫu"] {
        assert!(page.contains(item), "admin menu has {item}");
    }
    assert!(
        page.contains(r#"aria-current="page">"#),
        "the dashboard entry is current"
    );
    assert!(
        !page.contains("nhà cung cấp mẫu hiển thị"),
        "no demo banner without demo data"
    );

    register_provider(&e.state, "nongdan", "0901234567").await;
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    let res = send(&e.router, get_page("/admin/dashboard", Some(&provider))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_dashboard_warns_while_demo_listings_are_live() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    send(
        &e.router,
        post_form("/setup/seed", &[("action", "load")], Some(&admin)),
    )
    .await;
    let page = body_text(send(&e.router, get_page("/admin/dashboard", Some(&admin))).await).await;
    assert!(page.contains("nhà cung cấp mẫu hiển thị lẫn với dữ liệu thật"));
    assert!(page.contains(r#"href="/setup/seed""#));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_grant_on_the_grid_takes_effect_on_the_next_request_and_is_audited() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    let provider_role = role_id(&e, "provider").await;
    let path = format!("/admin/roles/{provider_role}/permissions");

    let roles = body_text(send(&e.router, get_page("/admin/roles", Some(&admin))).await).await;
    assert!(roles.contains("Quản trị viên") && roles.contains("Nhà cung cấp"));
    let grid = body_text(
        send(
            &e.router,
            get_page(&format!("/admin/roles/{provider_role}"), Some(&admin)),
        )
        .await,
    )
    .await;
    assert!(grid.contains(r#"name="perm.own.listing.create" checked"#));
    assert!(
        grid.contains(r#"name="perm.admin.dashboard.view">"#),
        "not granted yet"
    );

    // Give providers the dashboard, keeping what they had.
    let mut fields = current_grid(&e, "provider").await;
    fields.push("perm.admin.dashboard.view".into());
    let mut form: Vec<(&str, &str)> = fields.iter().map(|f| (f.as_str(), "on")).collect();
    form.push(("reason", "thử nghiệm"));
    let res = send(&e.router, post_form(&path, &form, Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // The provider's existing session sees it at once: no re-login.
    let res = send(&e.router, get_page("/admin/dashboard", Some(&provider))).await;
    assert_eq!(res.status(), StatusCode::OK);
    let page = body_text(res).await;
    assert!(
        page.contains("Dashboard") && !page.contains("Vai trò"),
        "menu shows only what was granted"
    );

    let audit: String =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "SELECT action || ' ' || reason AS line FROM audit_logs WHERE entity_type = 'role'",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "line")
            .unwrap();
    assert_eq!(
        audit,
        "change_role_permissions +admin.dashboard.view: thử nghiệm"
    );

    // Taking it away closes the page again.
    fields.retain(|f| f != "perm.admin.dashboard.view");
    let form: Vec<(&str, &str)> = fields.iter().map(|f| (f.as_str(), "on")).collect();
    send(&e.router, post_form(&path, &form, Some(&admin))).await;
    let res = send(&e.router, get_page("/admin/dashboard", Some(&provider))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_grid_refuses_self_lockout_unknown_keys_and_non_managers() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let admin_role = role_id(&e, "admin").await;
    let path = format!("/admin/roles/{admin_role}/permissions");
    let before = current_grid(&e, "admin").await;

    let without_manage: Vec<(&str, &str)> = before
        .iter()
        .filter(|f| *f != "perm.role.permission.manage")
        .map(|f| (f.as_str(), "on"))
        .collect();
    let res = send(&e.router, post_form(&path, &without_manage, Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert!(body_text(res)
        .await
        .contains("Không thể bỏ quyền quản lý phân quyền"));

    let res = send(
        &e.router,
        post_form(&path, &[("perm.ads.manage", "on")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res)
        .await
        .contains("Quyền ads.manage không tồn tại."));
    assert_eq!(current_grid(&e, "admin").await, before, "nothing changed");

    register_provider(&e.state, "nongdan", "0901234567").await;
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    let res = send(&e.router, post_form(&path, &[], Some(&provider))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(current_grid(&e, "admin").await, before);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_permission_matrix_lists_every_key_per_role() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let page = body_text(send(&e.router, get_page("/admin/permissions", Some(&admin))).await).await;
    assert!(page.contains("<code class=\"d-block small\">own.listing.create</code>"));
    let provider_role = role_id(&e, "provider").await;
    assert!(page.contains(&format!(
        r#"href="/admin/roles/{provider_role}">Nhà cung cấp</a>"#
    )));
    e.cleanup().await;
}
