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
    integration_tests::load_admin_units(&e).await;
    let (cookie, _) = provider(&e).await;

    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(html.contains("Vườn nongdan"));
    assert!(html.contains(r#"<ac-geolocate lat-target="pf-lat""#));
    assert!(html.contains("noindex"));

    let res = send(
        &e.router,
        post_form(
            "/provider/profile",
            &[("name", ""), ("phone", "12345"), ("coords", "10.4")],
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
        "one number alone is no point"
    );
    assert!(html.contains(r#"value="12345""#), "typed phone kept");

    let res = send(
        &e.router,
        post_form(
            "/provider/profile",
            &[
                ("name", "Vườn Út Hiền"),
                ("phone", "0901234567"),
                ("coords", "10,4048 106,118"),
            ],
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/provider/dashboard");
    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(html.contains("Vườn Út Hiền"));
    assert!(
        html.contains(r#"value="10.4048, 106.118""#),
        "location stored"
    );

    // A Google Maps link is read; a spot in the sea is refused.
    let save = |coords: &'static str| {
        send(
            &e.router,
            post_form(
                "/provider/profile",
                &[
                    ("name", "Vườn Út Hiền"),
                    ("phone", "0901234567"),
                    ("coords", coords),
                ],
                Some(&cookie),
            ),
        )
    };
    let res =
        save("https://www.google.com/maps/place/X/@10.3,106.0,15z/data=!3d10.41!4d106.12").await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(
        html.contains(r#"value="10.41, 106.12""#),
        "the place's own pin"
    );
    let res = save("10.0, 107.5").await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains("không thuộc xã, phường nào"));

    // The box emptied: the pin comes off the map.
    let res = send(
        &e.router,
        post_form(
            "/provider/profile",
            &[
                ("name", "Vườn Út Hiền"),
                ("phone", "0901234567"),
                ("coords", ""),
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
            "/provider/supplies/new",
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
            "/provider/supplies/new",
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

    let new_page = page(&e, "/provider/supplies/new", &cookie).await;
    assert!(new_page.contains(r#"enctype="multipart/form-data""#));
    assert!(
        !new_page.contains("capture="),
        "the gallery can be used too"
    );

    // Every box rule at once; the typed price is kept.
    let res = send(
        &e.router,
        post_multipart_form(
            "/provider/supplies/new",
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
            "/provider/supplies/new",
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

    let edit = format!("/provider/supplies/{id}/edit");
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
            &format!("/provider/supplies/{id}/delete"),
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
            "/provider/supplies/new",
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
            "/provider/supplies/new",
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
    let edit = format!("/provider/supplies/{id}/edit");
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
    assert!(html.contains("Tài khoản chưa gắn với nhà cung cấp nào"));
    let res = send(&e.router, get_page("/provider/dashboard", None)).await;
    assert_eq!(res.headers()[header::LOCATION], "/login");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_account_that_may_not_list_is_refused_before_its_photo_is_read() {
    let e = env_with_origin().await;
    // The admin holds no own.* permission and manages no business.
    let admin = install_admin(&e).await;
    let (category, _) = categories_of(&e, "product").await[0].clone();
    let (service_type, _) = categories_of(&e, "service").await[0].clone();
    // Not a picture: read, it would come back as a photo error (400).
    let junk = b"not a picture".to_vec();
    for (path, field) in [
        ("/provider/supplies/new", category.to_string()),
        ("/provider/services/new", service_type.to_string()),
    ] {
        let res = send(
            &e.router,
            post_multipart_form(
                path,
                &[("category_id", &field), ("name", "Thử")],
                Some(&junk),
                &admin,
            ),
        )
        .await;
        // 403 permission_denied, not the 400 of a bad photo.
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "{path}");
    }
    assert_eq!(count(&e, "SELECT count(*) FROM files").await, 0);
    e.cleanup().await;
}

/// Storage that refuses every upload: a storage outage, or a weak link.
struct DownStorage;

#[async_trait::async_trait]
impl application::ports::FileStorage for DownStorage {
    async fn put(&self, _key: &str, _bytes: Vec<u8>) -> domain::Result<()> {
        Err(domain::Error::internal("storage is down"))
    }
    async fn get(&self, _key: &str) -> domain::Result<Vec<u8>> {
        Err(domain::Error::internal("storage is down"))
    }
    async fn delete(&self, _key: &str) -> domain::Result<()> {
        Ok(())
    }
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_edit_whose_photo_cannot_be_stored_saves_nothing() {
    let e = env_with_origin().await;
    let (cookie, category) = provider(&e).await;
    let res = send(
        &e.router,
        post_multipart_form(
            "/provider/supplies/new",
            &[("category_id", &category), ("name", "Bưởi")],
            None,
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let id = count(&e, "SELECT max(id)::bigint FROM products").await;

    // The same database and session, behind storage that is down.
    let down = integration_tests::router_with(
        integration_tests::state_with_uploads(
            e.t.db.clone(),
            web::startup::Uploads {
                storage: std::sync::Arc::new(DownStorage),
                max_bytes: e.state.usecases.media.max_upload_bytes(),
            },
        ),
        web::routings::app_routing::RouterConfig {
            allowed_origins: vec![integration_tests::TEST_ORIGIN.to_owned()],
            ..Default::default()
        },
    );
    let res = send(
        &down,
        post_multipart_form(
            &format!("/provider/supplies/{id}/edit"),
            &[("category_id", &category), ("name", "Bưởi da xanh")],
            Some(&photo(32, 32)),
            &cookie,
        ),
    )
    .await;
    assert!(res.status().is_server_error(), "{}", res.status());
    // A server error: the error page, as for any form (`refused`).
    assert_eq!(
        count(
            &e,
            &format!("SELECT count(*) FROM products WHERE id = {id} AND name = 'Bưởi'")
        )
        .await,
        1,
        "the new name is not saved without its photo"
    );
    assert_eq!(count(&e, "SELECT count(*) FROM files").await, 0);
    e.cleanup().await;
}

/// One value of the first row of `sql`.
async fn value<T: sea_orm::TryGetable>(e: &Env, sql: &str) -> T {
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

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_ward_chosen_places_the_business_and_the_box_says_where_in_words() {
    let e = env_with_origin().await;
    integration_tests::load_admin_units(&e).await;
    let (cookie, _) = provider(&e).await;
    let ward: String = value(&e, "SELECT code FROM wards ORDER BY code LIMIT 1").await;
    let other: String = value(&e, "SELECT code FROM wards ORDER BY code DESC LIMIT 1").await;
    let label: String = value(
        &e,
        &format!(
            "SELECT w.full_name || ', ' || p.name FROM wards w \
             JOIN provinces p ON p.code = w.province_code WHERE w.code = '{ward}'"
        ),
    )
    .await;
    let point = |code: &str, how: &str| {
        format!("SELECT {how}(center::geometry)::float8 FROM wards WHERE code = '{code}'")
    };
    let (lat, lng): (f64, f64) = (
        value(&e, &point(&ward, "ST_Y")).await,
        value(&e, &point(&ward, "ST_X")).await,
    );
    let stored = |how: &str| format!("SELECT {how}(location::geometry)::float8 FROM businesses");
    let profile = |extra: &[(&'static str, String)]| {
        let mut pairs = vec![
            ("name", "Vườn Út Hiền".to_owned()),
            ("phone", "0901234567".to_owned()),
        ];
        pairs.extend(extra.iter().cloned());
        pairs
    };
    let save = |pairs: Vec<(&'static str, String)>| {
        let pairs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        send(
            &e.router,
            post_form("/provider/profile", &pairs, Some(&cookie)),
        )
    };

    // No place yet: the reminder on top, the box says so.
    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(html.contains("Bà con ở gần chưa thấy bạn"));
    assert!(html.contains("Chưa có vị trí"));
    assert!(html.contains(r#"data-tiles="/map/tiles/{z}/{x}/{y}""#));

    // A ward alone: the business stands at its centre, said to be approximate.
    let res = save(profile(&[
        ("ward", format!("w{ward}")),
        ("coords", String::new()),
    ]))
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let y: f64 = value(&e, &stored("ST_Y")).await;
    let x: f64 = value(&e, &stored("ST_X")).await;
    assert!(
        (y - lat).abs() < 1e-6 && (x - lng).abs() < 1e-6,
        "at the ward's centre"
    );
    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(html.contains(&label), "{label}");
    assert!(html.contains("Gần đúng: giữa xã"));
    assert!(!html.contains("Bà con ở gần chưa thấy bạn"));

    // A pin inside the chosen ward is kept as pinned.
    let inside = |how: &str| {
        format!("SELECT {how}(ST_PointOnSurface(boundary::geometry))::float8 FROM wards WHERE code = '{ward}'")
    };
    let (pin_lat, pin_lng): (f64, f64) = (
        value(&e, &inside("ST_Y")).await,
        value(&e, &inside("ST_X")).await,
    );
    let res = save(profile(&[
        ("ward", format!("w{ward}")),
        ("coords", format!("{pin_lat}, {pin_lng}")),
    ]))
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let y: f64 = value(&e, &stored("ST_Y")).await;
    assert!((y - pin_lat).abs() < 1e-6, "the pin is kept");
    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(html.contains("Đúng chỗ đã ghim."));

    // Another ward chosen while the old pin stays: the new ward's centre.
    let res = save(profile(&[
        ("ward", format!("w{other}")),
        ("coords", format!("{pin_lat}, {pin_lng}")),
    ]))
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let code: String = value(&e, "SELECT ward_code FROM businesses").await;
    assert_eq!(code, other, "moved to the ward chosen");

    // A ward nobody has: a message on the box, nothing saved.
    let res = save(profile(&[("ward", "w99999".to_owned())])).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains(r#"id="pf-ward-error""#));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_dropped_pin_is_named_by_its_ward_and_the_map_serves_only_the_region() {
    let e = env_with_origin().await;
    integration_tests::load_admin_units(&e).await;
    let ward: String = value(&e, "SELECT code FROM wards ORDER BY code LIMIT 1").await;
    let inside = |how: &str| {
        format!("SELECT {how}(ST_PointOnSurface(boundary::geometry))::float8 FROM wards WHERE code = '{ward}'")
    };
    let (lat, lng): (f64, f64) = (
        value(&e, &inside("ST_Y")).await,
        value(&e, &inside("ST_X")).await,
    );

    let res = send(
        &e.router,
        get_page(&format!("/api/places/ward?lat={lat}&lng={lng}"), None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let place = integration_tests::json_body(res).await;
    assert_eq!(place["area"], format!("w{ward}"));
    assert!(place["label"].as_str().is_some_and(|l| l.contains(", ")));

    // Paris is in no ward of ours.
    let res = send(
        &e.router,
        get_page("/api/places/ward?lat=48.85&lng=2.35", None),
    )
    .await;
    let place = integration_tests::json_body(res).await;
    assert_eq!(place["area"], "");

    // A tile of Paris is refused before any tile host is asked.
    let res = send(&e.router, get_page("/map/tiles/12/2073/1409", None)).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        integration_tests::json_body(res).await["code"],
        "map_tile_not_found"
    );
    e.cleanup().await;
}
