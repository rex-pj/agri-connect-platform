//! /admin/providers: the provider list, creating and editing a provider,
//! verification and account assignment.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, env_with_origin, get_page, install_admin, load_admin_units, post_form,
    register_provider, send, sign_in, Env,
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
            "INSERT INTO businesses (name, phone, email) \
             VALUES ('Máy cày Út Hiền', '0912345678', 'uthien@example.vn') RETURNING id"
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
    let detail = format!("/admin/providers/{id}");
    let action = format!("{detail}/verification");

    let list = body_text(send(&e.router, get_page("/admin/providers", Some(&admin))).await).await;
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
    let detail = format!("/admin/providers/{id}");
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
    let action = format!("/admin/providers/{id}/account");

    // The form starts with the business's own address.
    let page = body_text(
        send(
            &e.router,
            get_page(&format!("/admin/providers/{id}"), Some(&admin)),
        )
        .await,
    )
    .await;
    assert!(page.contains(r#"value="uthien@example.vn""#));

    // The email is checked first; a new account then needs its password.
    let res = send(
        &e.router,
        post_form(&action, &[("email", "uthien")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Email không hợp lệ"));
    assert!(html.contains(r#"value="uthien""#), "typed email kept");
    let res = send(
        &e.router,
        post_form(
            &action,
            &[("email", "uthien@example.vn"), ("password", "ngan")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Mật khẩu phải có ít nhất"));
    assert!(!html.contains(r#"value="ngan""#), "password never echoed");

    let res = send(
        &e.router,
        post_form(
            &action,
            &[("email", "UThien@example.vn"), ("password", "matkhau123")],
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
    assert_eq!(line, "assign_provider_account: uthien@example.vn");
    sign_in(&e, "uthien", "matkhau123").await;
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_business_goes_only_to_an_account_whose_email_was_confirmed() {
    // Anyone may sign up with a stranger's address: until its owner opened
    // the link, a business typed with that address must not go to it.
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let id = unclaimed_business(&e).await;
    let action = format!("/admin/providers/{id}/account");
    // A sign-up's own business is taken away, so only the email can refuse.
    register_provider(&e.state, "uthien", "0901234567").await;
    e.t.db
        .execute_unprepared(
            "UPDATE businesses SET owner_id = NULL WHERE name = 'Vườn uthien'",
        )
        .await
        .unwrap();
    let res = send(
        &e.router,
        post_form(&action, &[("email", "uthien@example.vn")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res)
        .await
        .contains("Tài khoản này chưa xác nhận email."));

    integration_tests::confirm_email(&e, "uthien").await;
    let res = send(
        &e.router,
        post_form(&action, &[("email", "uthien@example.vn")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "confirmed, it is linked");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_account_manages_one_business_and_admins_cannot_be_linked() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let id = unclaimed_business(&e).await;
    let action = format!("/admin/providers/{id}/account");

    // "nongdan" already manages the business it registered.
    let res = send(
        &e.router,
        post_form(&action, &[("email", "nongdan@example.vn")], Some(&admin)),
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
        post_form(&action, &[("email", "quantri@example.vn")], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // A provider without the assign permission cannot use the page at all.
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    let res = send(
        &e.router,
        get_page(&format!("/admin/providers/{id}"), Some(&provider)),
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
        .login(
            "quantri@example.vn".into(),
            "matkhau123".into(),
            integration_tests::CLIENT,
        )
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
    load_admin_units(&e).await;
    let admin = install_admin(&e).await;
    let list = body_text(send(&e.router, get_page("/admin/providers", Some(&admin))).await).await;
    assert!(
        list.contains(r##"href="#them-nha-cung-cap""##),
        "the create sheet is offered"
    );

    // A broken rule: the sheet comes back open with the values as typed.
    let res = send(
        &e.router,
        post_form(
            "/admin/providers",
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
            "/admin/providers",
            &[
                ("name", "Drone Ba Tu"),
                ("phone", "0912345678"),
                ("address", "Cai Lay, Dong Thap"),
                ("coords", "10.4048, 106.118"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let detail = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    let id = scalar(&e, "SELECT id::text AS id FROM businesses", "id").await;
    assert_eq!(detail, format!("/admin/providers/{id}"));
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
            "/admin/providers",
            &[("name", "X"), ("phone", "0912345678")],
            Some(&provider),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_provider_page_opens_with_its_summary_area_and_activity() {
    let e = env_with_origin().await;
    load_admin_units(&e).await;
    let admin = install_admin(&e).await;
    let id = unclaimed_business(&e).await;
    let detail = format!("/admin/providers/{id}");
    let page = || async { body_text(send(&e.router, get_page(&detail, Some(&admin))).await).await };

    // No position, no account, no taps yet: each said in the summary.
    let html = page().await;
    assert!(html.contains("Chưa có vị trí: bà con không thấy ở trang Gần tôi."));
    assert!(html.contains("Chưa có tài khoản"));
    assert!(html.contains("Gán tài khoản"));
    assert!(html.contains("Lượt gọi + Zalo (30 ngày)"));
    assert!(html.contains(r#"href="https://zalo.me/0912345678""#));
    // The overview first; the profile form on a tab of its own.
    assert!(html.contains(r#"id="summary-title""#));
    assert!(!html.contains(r#"id="ho-so-title""#));
    let tab = format!("{detail}?tab=profile");
    let profile = body_text(send(&e.router, get_page(&tab, Some(&admin))).await).await;
    assert!(profile.contains(r#"id="ho-so-title""#));

    // Placed in a ward, tapped once: its area by name, the map, the figure.
    let ward = scalar(&e, "SELECT code FROM wards ORDER BY code LIMIT 1", "code").await;
    let area = scalar(
        &e,
        &format!(
            "SELECT w.full_name || ', ' || p.name AS area FROM wards w \
             JOIN provinces p ON p.code = w.province_code WHERE w.code = '{ward}'"
        ),
        "area",
    )
    .await;
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE businesses SET ward_code = '{ward}', \
             location = ST_SetSRID(ST_MakePoint(105.9, 10.3), 4326)::geography WHERE id = {id}; \
             INSERT INTO call_clicks (business_id, channel) VALUES ({id}, 'call');"
        ))
        .await
        .unwrap();
    let html = page().await;
    assert!(html.contains(&area), "area {area}");
    assert!(html.contains(r#"href="https://www.google.com/maps?q=10.3,105.9""#));
    assert!(html.contains(r#"<p class="stat-card__value">1</p>"#));
    assert!(!html.contains("Chưa có vị trí"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_provider_page_says_who_changed_it_and_why() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let id = unclaimed_business(&e).await;
    let detail = format!("/admin/providers/{id}");
    let page = || async { body_text(send(&e.router, get_page(&detail, Some(&admin))).await).await };

    let history = format!("{detail}?tab=history");
    let html = body_text(send(&e.router, get_page(&history, Some(&admin))).await).await;
    assert!(html.contains("Chưa có thay đổi nào được ghi lại."));
    let html = page().await;
    assert!(
        html.contains("Trước khi cấp, kiểm tra:"),
        "the checklist while unverified"
    );

    let post = |path: String, pairs: &'static [(&'static str, &'static str)]| {
        send(&e.router, post_form(&path, pairs, Some(&admin)))
    };
    post(
        format!("{detail}/status"),
        &[
            ("is_active", "false"),
            ("reason", "Số điện thoại không liên lạc được"),
        ],
    )
    .await;
    let html = page().await;
    assert!(html.contains("Tắt nhà cung cấp"));
    assert!(html.contains("Số điện thoại không liên lạc được"));

    post(format!("{detail}/verification"), &[("is_verified", "true")]).await;
    let html = page().await;
    assert!(html.contains("Cấp huy hiệu xác minh"));
    assert!(
        !html.contains("Trước khi cấp, kiểm tra:"),
        "no checklist once verified"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_managing_account_is_removed_behind_a_confirmation() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    register_provider(&e.state, "chuvuon", "0901234567").await;
    let id: i32 = scalar(
        &e,
        "SELECT id::text AS id FROM businesses WHERE name = 'Vườn chuvuon'",
        "id",
    )
    .await
    .parse()
    .unwrap();
    let detail = format!("/admin/providers/{id}");
    let page = || async { body_text(send(&e.router, get_page(&detail, Some(&admin))).await).await };

    let html = page().await;
    assert!(html.contains("Đổi tài khoản"));
    assert!(html.contains("Email của tài khoản chưa xác nhận"));
    let html = body_text(
        send(
            &e.router,
            get_page(&format!("{detail}?tab=offers"), Some(&admin)),
        )
        .await,
    )
    .await;
    assert!(html.contains(&format!(
        r#"href="/admin/services?provider_id={id}#them-dich-vu""#
    )));
    assert!(html.contains(&format!(
        r#"href="/admin/supplies?provider_id={id}#them-vat-tu""#
    )));

    let res = send(
        &e.router,
        post_form(
            &format!("{detail}/account/delete"),
            &[("reason", "Chủ đã bán vườn")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        scalar(
            &e,
            &format!("SELECT (owner_id IS NULL)::text AS gone FROM businesses WHERE id = {id}"),
            "gone"
        )
        .await,
        "true"
    );
    let html = page().await;
    assert!(html.contains("Gán tài khoản"));
    let html = body_text(
        send(
            &e.router,
            get_page(&format!("{detail}?tab=history"), Some(&admin)),
        )
        .await,
    )
    .await;
    assert!(html.contains("Gỡ tài khoản"), "in its history");
    assert!(html.contains("Chủ đã bán vườn"));
    // Twice: nothing left to remove.
    let res = send(
        &e.router,
        post_form(&format!("{detail}/account/delete"), &[], Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    e.cleanup().await;
}
