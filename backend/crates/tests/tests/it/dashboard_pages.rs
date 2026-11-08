//! /provider/dashboard and the product forms: profile, products with photos,
//! and the rule that a refused photo saves nothing. Database tests need
//! TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, categories_of, env_with_origin, get_page, install_admin, photo, post_form,
    post_multipart_form, register_provider, send, sign_in, Env,
};
use sea_orm::{ConnectionTrait, Statement};

async fn count(e: &Env, sql: &str) -> i64 {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            sql.to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index(0)
        .unwrap()
}

async fn page(e: &Env, path: &str, cookie: &str) -> String {
    body_text(send(&e.router, get_page(path, Some(cookie))).await).await
}

/// The provider "nongdan", signed in, and a product category id.
async fn provider(e: &Env) -> (String, String) {
    register_provider(&e.state, "nongdan", "0901234567").await;
    let cookie = sign_in(e, "nongdan", "matkhau123").await;
    let (category, _) = categories_of(e, "product").await[0].clone();
    (cookie, category.to_string())
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_profile_reports_every_rule_and_saves_a_location() {
    let e = env_with_origin().await;
    let (cookie, _) = provider(&e).await;

    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(html.contains("Vườn nongdan"));
    assert!(html.contains(r#"<ac-geolocate lat-target="pf-lat""#));
    assert!(html.contains("noindex"));

    let res = send(
        &e.router,
        post_form(
            "/provider/profile",
            &[("name", ""), ("phone", "12345"), ("lat", "10.4")],
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains(r#"id="pf-name-error""#));
    assert!(html.contains(r#"id="pf-phone-error""#));
    assert!(
        html.contains(r#"id="pf-coord-error""#),
        "one coordinate alone is refused"
    );
    assert!(html.contains(r#"value="12345""#), "typed phone kept");

    let res = send(
        &e.router,
        post_form(
            "/provider/profile",
            &[
                ("name", "Vườn Út Hiền"),
                ("phone", "0901234567"),
                ("lat", "10,4048"),
                ("lng", "106.118"),
            ],
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/provider/dashboard");
    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(html.contains("Vườn Út Hiền"));
    assert!(html.contains(r#"value="10.4048""#), "location stored");

    // Both boxes emptied: the pin comes off the map.
    let res = send(
        &e.router,
        post_form(
            "/provider/profile",
            &[
                ("name", "Vườn Út Hiền"),
                ("phone", "0901234567"),
                ("lat", ""),
                ("lng", ""),
            ],
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        count(
            &e,
            "SELECT count(*) FROM businesses WHERE location IS NOT NULL"
        )
        .await,
        0,
        "location removed"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_new_product_and_its_photo_are_saved_together() {
    let e = env_with_origin().await;
    let (cookie, category) = provider(&e).await;
    // A role that may add listings but not edit them: the photo belongs to
    // the new product, so adding it needs no edit permission.
    e.t.db
        .execute_unprepared(
            "DELETE FROM role_permissions WHERE permission_id = \
             (SELECT id FROM permissions WHERE code = 'own.listing.update')",
        )
        .await
        .unwrap();

    let photo = photo(64, 48);
    let res = send(
        &e.router,
        post_multipart_form(
            "/provider/products/new",
            &[("category_id", &category), ("name", "Sầu riêng Ri6")],
            Some(&photo),
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        count(
            &e,
            "SELECT count(*) FROM products WHERE image_file_id IS NOT NULL"
        )
        .await,
        1
    );
    assert_eq!(count(&e, "SELECT count(*) FROM files").await, 1);

    // A refused form stores no photo.
    let res = send(
        &e.router,
        post_multipart_form(
            "/provider/products/new",
            &[("category_id", &category), ("name", " ")],
            Some(&photo),
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(count(&e, "SELECT count(*) FROM files").await, 1);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_product_is_added_with_its_photo_edited_and_deleted() {
    let e = env_with_origin().await;
    let (cookie, category) = provider(&e).await;

    let new_page = page(&e, "/provider/products/new", &cookie).await;
    assert!(new_page.contains(r#"enctype="multipart/form-data""#));
    assert!(
        !new_page.contains("capture="),
        "the gallery can be used too"
    );

    // Every box rule at once; the typed price is kept.
    let res = send(
        &e.router,
        post_multipart_form(
            "/provider/products/new",
            &[("category_id", "x"), ("name", " "), ("price", "85.000")],
            None,
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains(r#"id="pr-name-error""#));
    assert!(html.contains(r#"id="pr-category-error""#));
    assert!(html.contains(r#"value="85.000""#));

    let photo = photo(64, 48);
    let res = send(
        &e.router,
        post_multipart_form(
            "/provider/products/new",
            &[
                ("category_id", &category),
                ("name", "Sầu riêng Ri6"),
                ("price", "85.000"),
                ("unit", "kg"),
            ],
            Some(&photo),
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/provider/dashboard");
    let id = count(&e, "SELECT max(id)::bigint FROM products").await;
    assert_eq!(
        count(
            &e,
            "SELECT count(*) FROM products WHERE price = 85000 AND image_file_id IS NOT NULL"
        )
        .await,
        1,
        "price parsed with its separators, photo attached"
    );

    let edit = format!("/provider/products/{id}/edit");
    let html = page(&e, &edit, &cookie).await;
    assert!(html.contains(r#"value="Sầu riêng Ri6""#));
    assert!(html.contains("Ảnh hiện tại"));

    // No new photo: the old one stays.
    let res = send(
        &e.router,
        post_multipart_form(
            &edit,
            &[
                ("category_id", &category),
                ("name", "Sầu riêng Ri6 loại 1"),
                ("price", ""),
            ],
            None,
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        count(
            &e,
            "SELECT count(*) FROM products WHERE price IS NULL AND image_file_id IS NOT NULL"
        )
        .await,
        1
    );

    let res = send(
        &e.router,
        post_form(
            &format!("/provider/products/{id}/delete"),
            &[],
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let res = send(&e.router, get_page(&edit, Some(&cookie))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_refused_photo_saves_nothing() {
    let e = env_with_origin().await;
    let (cookie, category) = provider(&e).await;

    let res = send(
        &e.router,
        post_multipart_form(
            "/provider/products/new",
            &[("category_id", &category), ("name", "Sầu riêng")],
            Some(b"not an image at all"),
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(
        html.contains(r#"id="pr-image-error""#),
        "the refusal is under the photo box"
    );
    assert!(html.contains(r#"value="Sầu riêng""#));
    assert_eq!(count(&e, "SELECT count(*) FROM products").await, 0);
    assert_eq!(count(&e, "SELECT count(*) FROM files").await, 0);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn another_providers_product_looks_missing() {
    let e = env_with_origin().await;
    let (cookie, category) = provider(&e).await;
    let res = send(
        &e.router,
        post_multipart_form(
            "/provider/products/new",
            &[("category_id", &category), ("name", "Bưởi")],
            None,
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let id = count(&e, "SELECT max(id)::bigint FROM products").await;

    register_provider(&e.state, "hangxom", "0907654321").await;
    let other = sign_in(&e, "hangxom", "matkhau123").await;
    let edit = format!("/provider/products/{id}/edit");
    let res = send(&e.router, get_page(&edit, Some(&other))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let res = send(
        &e.router,
        post_multipart_form(
            &edit,
            &[("category_id", &category), ("name", "Của tôi")],
            Some(&photo(32, 32)),
            &other,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        count(&e, "SELECT count(*) FROM files").await,
        0,
        "never decoded"
    );

    // An admin has no business: the dashboard says so instead of failing.
    let admin = install_admin(&e).await;
    let html = page(&e, "/provider/dashboard", &admin).await;
    assert!(html.contains("Tài khoản chưa gắn với cửa hàng nào"));
    let res = send(&e.router, get_page("/provider/dashboard", None)).await;
    assert_eq!(res.headers()[header::LOCATION], "/login");
    e.cleanup().await;
}
