//! /admin/categories and /admin/locations: create, edit, hide and delete
//! through page forms, with every broken rule reported on its field.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{
    body_text, env_with_origin, get_page, install_admin, post_form, register_provider, send,
    sign_in, Env,
};
use sea_orm::{ConnectionTrait, Statement};

fn location_of(res: &axum::response::Response<Body>) -> String {
    res.headers()[header::LOCATION].to_str().unwrap().to_owned()
}

async fn page(e: &Env, path: &str, cookie: &str) -> String {
    body_text(send(&e.router, get_page(path, Some(cookie))).await).await
}

async fn last_audit(e: &Env, entity_type: &str) -> String {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            format!(
                "SELECT action || ': ' || coalesce(reason, '') AS line FROM audit_logs \
                 WHERE entity_type = '{entity_type}' ORDER BY id DESC LIMIT 1"
            ),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "line")
        .unwrap()
}

const NEW_CATEGORY: &[(&str, &str)] = &[
    ("category_type", "service"),
    ("name", "Bơm nước"),
    ("slug", "bom-nuoc"),
    ("icon", "fa-solid fa-faucet"),
    ("color", ""),
    ("description", "Bơm tưới, bơm tiêu"),
    ("display_order", "12"),
];

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_category_form_reports_every_broken_rule_and_keeps_what_was_typed() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    let res = send(
        &e.router,
        post_form(
            "/admin/categories",
            &[
                ("category_type", "weather"),
                ("name", "  "),
                ("slug", "Bơm Nước"),
                ("description", "Tưới tiêu"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Vui lòng nhập tên."));
    assert!(html.contains("Đường dẫn chỉ gồm chữ thường không dấu"));
    assert!(html.contains("Loại danh mục không hợp lệ."));
    assert!(html.contains(r#"value="Bơm Nước""#), "typed slug kept");
    assert!(
        html.contains(">Tưới tiêu</textarea>"),
        "typed description kept"
    );

    let res = send(
        &e.router,
        post_form(
            "/admin/categories",
            &[
                ("category_type", "service"),
                ("name", "Bơm nước"),
                ("slug", "bom-nuoc"),
                ("display_order", "mười"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains("Nhập một số nguyên hợp lệ."));

    // A slug already in use loses to the UNIQUE constraint, on its field.
    let res = send(
        &e.router,
        post_form(
            "/admin/categories",
            &[
                ("category_type", "service"),
                ("name", "Múc bùn 2"),
                ("slug", "muc-bun"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Đường dẫn này đã được dùng cho danh mục khác."));
    assert!(
        html.contains(r#"id="new-slug-error""#),
        "shown under the slug"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_category_is_created_edited_and_hidden_from_the_public_pages() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    let res = send(
        &e.router,
        post_form("/admin/categories", NEW_CATEGORY, Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let detail = location_of(&res);
    assert!(detail.starts_with("/admin/categories/"));
    let mut cookie = admin.clone();
    for c in res.headers().get_all(header::SET_COOKIE) {
        if let Some(pair) = c.to_str().unwrap().split(';').next() {
            cookie.push_str("; ");
            cookie.push_str(pair);
        }
    }
    let html = page(&e, &detail, &cookie).await;
    assert!(html.contains("Đã thêm danh mục."), "flash shown once");
    assert!(html.contains(r#"value="bom-nuoc""#));
    assert!(page(&e, "/admin/categories", &admin)
        .await
        .contains(&format!(r#"href="{detail}""#)));

    let mut edited = NEW_CATEGORY.to_vec();
    edited[1] = ("name", "Bơm nước tưới");
    let res = send(&e.router, post_form(&detail, &edited, Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let res = send(
        &e.router,
        post_form(
            &format!("{detail}/status"),
            &[("is_active", "false"), ("reason", "trùng danh mục")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        last_audit(&e, "category").await,
        "hide_category: trùng danh mục"
    );
    let public = body_text(
        send(
            &e.router,
            Request::get("/api/explore/categories?type=service")
                .body(Body::empty())
                .unwrap(),
        )
        .await,
    )
    .await;
    assert!(!public.contains("bom-nuoc"), "hidden from the public");
    let html = page(&e, &detail, &admin).await;
    assert!(html.contains("Bơm nước tưới"));
    assert!(html.contains("Hiển thị lại"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_catalogue_pages_need_their_permissions() {
    let e = env_with_origin().await;
    install_admin(&e).await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let provider = sign_in(&e, "nongdan", "matkhau123").await;

    for path in ["/admin/categories", "/admin/locations"] {
        let res = send(&e.router, get_page(path, Some(&provider))).await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "{path}");
    }
    let res = send(
        &e.router,
        post_form("/admin/categories", NEW_CATEGORY, Some(&provider)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = send(&e.router, get_page("/admin/locations", None)).await;
    assert_eq!(res.headers()[header::LOCATION], "/login");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_location_is_validated_created_hidden_and_deleted() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    let res = send(
        &e.router,
        post_form(
            "/admin/locations",
            &[("label", ""), ("lat", "mười"), ("lng", "106,1")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Vui lòng nhập tên khu vực."));
    assert!(html.contains("Toạ độ không hợp lệ."));
    assert!(html.contains(r#"value="mười""#), "typed value kept");

    let fields = [
        ("label", "Mỹ Phong"),
        ("lat", "10,4048"),
        ("lng", "106.118"),
        ("display_order", "3"),
    ];
    let res = send(
        &e.router,
        post_form("/admin/locations", &fields, Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let detail = location_of(&res);
    assert!(page(&e, &detail, &admin)
        .await
        .contains(r#"value="10.4048""#));

    let res = send(
        &e.router,
        post_form("/admin/locations", &fields, Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains("Đã có khu vực với tên này."));

    let res = send(
        &e.router,
        post_form(
            &format!("{detail}/status"),
            &[("is_active", "false")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let public = body_text(
        send(
            &e.router,
            Request::get("/api/explore/locations")
                .body(Body::empty())
                .unwrap(),
        )
        .await,
    )
    .await;
    assert!(!public.contains("Mỹ Phong"), "hidden from the explore page");

    let res = send(
        &e.router,
        post_form(
            &format!("{detail}/delete"),
            &[("reason", "nhập nhầm")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&res), "/admin/locations");
    assert_eq!(
        last_audit(&e, "location").await,
        "delete_location: nhập nhầm"
    );
    let res = send(&e.router, get_page(&detail, Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    e.cleanup().await;
}
