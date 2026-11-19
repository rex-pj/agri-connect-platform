//! /admin/supplies, /admin/services, /admin/inquiries and /admin/reviews:
//! moderation through page forms that land back on the same list page.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, categories_of, create_product, env_with_origin, get_page, install_admin, photo,
    post_form, post_multipart_form, register_provider, send, sign_in, Env,
};
use sea_orm::{ConnectionTrait, Statement};

async fn one_i32(e: &Env, sql: &str) -> i32 {
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

async fn one_text(e: &Env, sql: &str) -> String {
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

/// An admin, and the provider "nongdan" with its business id.
async fn setup(e: &Env) -> (String, String, i32) {
    let admin = install_admin(e).await;
    let token = register_provider(&e.state, "nongdan", "0901234567").await;
    let business = one_i32(e, "SELECT id FROM businesses WHERE name = 'Vườn nongdan'").await;
    (admin, token, business)
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_product_is_hidden_and_deleted_behind_confirmations_on_its_list_page() {
    let e = env_with_origin().await;
    let (admin, token, business) = setup(&e).await;
    let (category, _) = categories_of(&e, "product").await[0].clone();
    let id = create_product(
        &e,
        &token,
        &format!(r#"{{"category_id":{category},"name":"Sầu riêng Ri6"}}"#),
    )
    .await;
    let list = format!("/admin/supplies?provider_id={business}");

    let html = page(&e, &list, &admin).await;
    assert!(html.contains("Vườn nongdan · 1 vật tư"));
    assert!(!html.contains(r#"id="xac-nhan""#), "no confirmation yet");
    let html = page(&e, &format!("{list}&hide={id}"), &admin).await;
    assert!(html.contains(r#"data-open="true""#));
    assert!(html.contains("Ẩn vật tư Sầu riêng Ri6?"));
    assert!(
        html.contains(&format!(r#"name="provider_id" value="{business}""#)),
        "the confirmation keeps the list position"
    );

    let res = send(
        &e.router,
        post_form(
            &format!("/admin/supplies/{id}/status"),
            &[
                ("is_active", "false"),
                ("reason", "ảnh không thật"),
                ("back", "list"),
                ("provider_id", &business.to_string()),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], list.as_str());
    assert_eq!(
        one_text(
            &e,
            "SELECT action || ': ' || reason FROM audit_logs WHERE action = 'hide_product'"
        )
        .await,
        "hide_product: ảnh không thật"
    );

    // A posted position that is not a number is dropped, never followed.
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/supplies/{id}/delete"),
            &[
                ("reason", "trùng"),
                ("back", "list"),
                ("provider_id", "//evil.example"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/admin/supplies");
    assert!(page(&e, &list, &admin)
        .await
        .contains("Chưa có vật tư nào."));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn services_are_created_for_a_provider_edited_and_deleted() {
    let e = env_with_origin().await;
    let (admin, _, business) = setup(&e).await;
    let (category, _) = categories_of(&e, "service").await[0].clone();
    let list = format!("/admin/services?provider_id={business}");

    let all = page(&e, "/admin/services", &admin).await;
    assert!(
        !all.contains(r#"id="them-dich-vu""#),
        "create needs a provider"
    );
    assert!(page(&e, &list, &admin)
        .await
        .contains(r#"id="them-dich-vu""#));

    let res = send(
        &e.router,
        post_form(
            "/admin/services",
            &[
                ("business_id", &business.to_string()),
                ("category_id", "abc"),
                ("name", " "),
                ("price_note", "Từ 500.000đ"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Vui lòng nhập tên."));
    assert!(
        html.contains(r#"id="new-category-error""#),
        "category on its field"
    );
    assert!(html.contains(r#"value="Từ 500.000đ""#), "typed price kept");

    let category = category.to_string();
    let fields = [
        ("business_id", business.to_string()),
        ("category_id", category.clone()),
        ("name", "Múc bùn".to_owned()),
        ("price_note", String::new()),
        ("description", String::new()),
    ];
    let pairs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let res = send(
        &e.router,
        post_form("/admin/services", &pairs, Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let detail = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    assert!(page(&e, &detail, &admin)
        .await
        .contains(&format!(r#"<option value="{category}" selected>"#)));

    let res = send(
        &e.router,
        post_multipart_form(
            &detail,
            &[("category_id", &category), ("name", "Múc bùn vườn")],
            None,
            &admin,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // From the list's row: hide behind a confirmation, back on the same list.
    let id = detail.rsplit('/').next().unwrap();
    let html = page(&e, &format!("{list}&hide={id}"), &admin).await;
    assert!(html.contains("Ẩn dịch vụ Múc bùn vườn?"));
    assert!(html.contains(r#"name="back" value="list""#));
    let res = send(
        &e.router,
        post_form(
            &format!("{detail}/status"),
            &[
                ("is_active", "false"),
                ("back", "list"),
                ("provider_id", &business.to_string()),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.headers()[header::LOCATION], list.as_str());
    assert!(page(&e, &format!("{list}&delete={id}"), &admin)
        .await
        .contains("Xoá dịch vụ Múc bùn vườn?"));

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
    assert_eq!(res.headers()[header::LOCATION], list.as_str());
    let res = send(&e.router, get_page(&detail, Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn inquiry_status_and_review_visibility_are_set_from_their_lists() {
    let e = env_with_origin().await;
    let (admin, _, business) = setup(&e).await;
    let inquiry = one_i32(
        &e,
        &format!(
            "INSERT INTO inquiries (business_id, topic, requester_name, requester_phone) \
             VALUES ({business}, 'Múc bùn', 'Chú Tư', '0987654321') RETURNING id"
        ),
    )
    .await;
    let review = one_i32(
        &e,
        &format!(
            "INSERT INTO reviews (business_id, inquiry_id, rating, comment, reviewer_name, \
             reviewer_phone_hash, is_visible) \
             VALUES ({business}, {inquiry}, 2, 'Làm ẩu', 'Chú Tư', \
             encode(sha256('0987654321'), 'hex'), TRUE) RETURNING id"
        ),
    )
    .await;

    // The status is a badge; the row offers the next steps only, spam
    // through its confirmation.
    let html = page(&e, "/admin/inquiries", &admin).await;
    assert!(!html.contains("<select"));
    assert!(html.contains(r#"<span class="badge text-bg-info">Mới</span>"#));
    assert!(html.contains(r#"name="status" value="contacted""#));
    assert!(html.contains(r#"name="status" value="closed""#));
    assert!(!html.contains(r#"name="status" value="new""#));
    assert!(html.contains(&format!("/admin/inquiries?spam={inquiry}#xac-nhan")));
    assert!(!html.contains("Đánh dấu rác yêu cầu của Chú Tư?"));

    // The chips narrow the list to one status; an unknown one is the whole list.
    let html = page(&e, "/admin/inquiries?state=closed", &admin).await;
    assert!(!html.contains("Chú Tư"));
    assert!(html.contains("Không có yêu cầu nào ở trạng thái này."));
    assert!(html.contains(r#"href="/admin/inquiries?state=closed" aria-current="true""#));
    let html = page(&e, "/admin/inquiries?state=new", &admin).await;
    assert!(html.contains("Chú Tư"));
    assert!(
        html.contains(r#"name="state" value="new""#),
        "the forms keep it"
    );
    assert!(page(&e, "/admin/inquiries?state=xoa", &admin)
        .await
        .contains("Chú Tư"));

    // Narrowed to one provider: its rows only, and the forms keep the filter.
    let own = format!("/admin/inquiries?provider_id={business}");
    let html = page(&e, &own, &admin).await;
    assert!(html.contains("Vườn nongdan · 1 yêu cầu"));
    assert!(html.contains(&format!(r#"name="provider_id" value="{business}""#)));
    let other = business + 1000;
    for list in ["inquiries", "reviews"] {
        let html = page(&e, &format!("/admin/{list}?provider_id={other}"), &admin).await;
        assert!(!html.contains("Chú Tư"), "{list}: another provider's rows");
        assert!(html.contains(r#"class="admin-filter""#));
    }
    assert!(page(
        &e,
        &format!("/admin/reviews?provider_id={business}"),
        &admin
    )
    .await
    .contains("Vườn nongdan · 1 đánh giá"));

    let res = send(
        &e.router,
        post_form(
            &format!("/admin/inquiries/{inquiry}/status"),
            &[
                ("status", "contacted"),
                ("page", "1"),
                ("provider_id", &business.to_string()),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], own.as_str());
    assert_eq!(
        one_text(
            &e,
            &format!("SELECT status FROM inquiries WHERE id = {inquiry}")
        )
        .await,
        "contacted"
    );
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/inquiries/{inquiry}/status"),
            &[("status", "archived")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Spam asks first, with a reason for the audit log; the list's status
    // filter is kept on the way back.
    let html = page(&e, &format!("/admin/inquiries?spam={inquiry}"), &admin).await;
    assert!(html.contains("Đánh dấu rác yêu cầu của Chú Tư?"));
    assert!(html.contains(r#"name="reason""#));
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/inquiries/{inquiry}/status"),
            &[
                ("status", "spam"),
                ("reason", "số lạ"),
                ("state", "contacted"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        res.headers()[header::LOCATION],
        "/admin/inquiries?state=contacted"
    );
    assert_eq!(
        one_text(
            &e,
            &format!(
                "SELECT reason FROM audit_logs WHERE entity_type = 'inquiry'                  AND entity_id = {inquiry} ORDER BY id DESC LIMIT 1"
            )
        )
        .await,
        "spam: số lạ"
    );
    // A spam one only reopens.
    let html = page(&e, "/admin/inquiries", &admin).await;
    assert!(html.contains(r#"name="status" value="new""#));
    assert!(!html.contains(&format!("?spam={inquiry}")));

    let html = page(&e, &format!("/admin/reviews?hide={review}"), &admin).await;
    assert!(html.contains("Ẩn đánh giá của Chú Tư?"));
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/reviews/{review}/visibility"),
            &[("is_visible", "false"), ("reason", "không phải khách")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        one_text(
            &e,
            &format!("SELECT is_visible::text FROM reviews WHERE id = {review}")
        )
        .await,
        "false"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn moderation_pages_need_their_permissions() {
    let e = env_with_origin().await;
    setup(&e).await;
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    for path in [
        "/admin/supplies",
        "/admin/services",
        "/admin/inquiries",
        "/admin/reviews",
    ] {
        let res = send(&e.router, get_page(path, Some(&provider))).await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "{path}");
    }
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_product_is_edited_hidden_and_deleted_on_its_own_admin_page() {
    let e = env_with_origin().await;
    let (admin, token, business) = setup(&e).await;
    let (category, _) = categories_of(&e, "product").await[0].clone();
    let id = create_product(
        &e,
        &token,
        &format!(r#"{{"category_id":{category},"name":"Sầu riêng Ri6","price":85000}}"#),
    )
    .await;
    let own = format!("/admin/supplies/{id}");

    let list = page(&e, "/admin/supplies", &admin).await;
    assert!(
        list.contains(&format!(r#"href="{own}""#)),
        "rows open the page"
    );
    let html = page(&e, &own, &admin).await;
    assert!(html.contains(r#"value="Sầu riêng Ri6""#));
    assert!(html.contains(r#"value="85000""#));

    // The owner's rules: a blank name is refused in place, typed values kept.
    // The form carries a photo box, so it is multipart like the owner's.
    assert!(html.contains(r#"enctype="multipart/form-data""#));
    let category = category.to_string();
    let form = |name: &'static str, price: &'static str| {
        post_multipart_form(
            &own,
            &[
                ("category_id", category.as_str()),
                ("name", name),
                ("price", price),
                ("unit", "kg"),
            ],
            None,
            &admin,
        )
    };
    let res = send(&e.router, form("", "90.000")).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains(r#"value="90.000""#));

    let res = send(&e.router, form("Sầu riêng Monthong", "90.000")).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], own.as_str());
    assert_eq!(
        one_text(&e, &format!("SELECT name FROM products WHERE id = {id}")).await,
        "Sầu riêng Monthong"
    );
    assert_eq!(
        one_i32(
            &e,
            &format!("SELECT price::int FROM products WHERE id = {id}")
        )
        .await,
        90000
    );
    assert_eq!(
        one_text(
            &e,
            "SELECT action FROM audit_logs WHERE entity_type = 'product' ORDER BY id DESC LIMIT 1"
        )
        .await,
        "update_product"
    );

    // Its own page's actions land back on it; a deletion on the provider's
    // products.
    let res = send(
        &e.router,
        post_form(
            &format!("{own}/status"),
            &[("is_active", "false")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.headers()[header::LOCATION], own.as_str());
    let res = send(
        &e.router,
        post_form(&format!("{own}/delete"), &[], Some(&admin)),
    )
    .await;
    assert_eq!(
        res.headers()[header::LOCATION],
        format!("/admin/supplies?provider_id={business}").as_str()
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn admin_lists_find_records_by_name_without_diacritics() {
    let e = env_with_origin().await;
    let (admin, token, _) = setup(&e).await;
    let (category, _) = categories_of(&e, "product").await[0].clone();
    for name in ["Sầu riêng Ri6", "Phân bón NPK"] {
        create_product(
            &e,
            &token,
            &format!(r#"{{"category_id":{category},"name":"{name}"}}"#),
        )
        .await;
    }

    let html = page(&e, "/admin/supplies?q=sau+rieng", &admin).await;
    assert!(html.contains("Sầu riêng Ri6") && !html.contains("Phân bón NPK"));
    assert!(
        html.contains(r#"name="q" value="sau rieng""#),
        "the box keeps it"
    );
    // A row's confirmation keeps the search, so the redirect lands on it.
    let id = one_i32(&e, "SELECT id FROM products WHERE name = 'Sầu riêng Ri6'").await;
    let html = page(
        &e,
        &format!("/admin/supplies?q=sau+rieng&hide={id}"),
        &admin,
    )
    .await;
    assert!(html.contains(r#"name="q" value="sau rieng""#));
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/supplies/{id}/status"),
            &[("is_active", "false"), ("back", "list"), ("q", "sau rieng")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(
        res.headers()[header::LOCATION],
        "/admin/supplies?q=sau+rieng"
    );

    // The shop's products by the shop's name; nothing found says so.
    assert!(page(&e, "/admin/supplies?q=vuon+nongdan", &admin)
        .await
        .contains("Phân bón NPK"));
    assert!(page(&e, "/admin/supplies?q=khong-co", &admin)
        .await
        .contains("Không có kết quả cho “khong-co”"));
    // Accounts by username, shops by name.
    let users = page(&e, "/admin/users?q=NONGDAN", &admin).await;
    assert!(users.contains("nongdan"));
    assert!(page(&e, "/admin/providers?q=vuon", &admin)
        .await
        .contains("Vườn nongdan"));
    assert!(!page(&e, "/admin/providers?q=khong-co", &admin)
        .await
        .contains("Vườn nongdan"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_supply_is_listed_for_a_provider_and_its_photo_set_and_removed() {
    let e = env_with_origin().await;
    let (admin, _, business) = setup(&e).await;
    let (category, _) = categories_of(&e, "product").await[0].clone();
    let list = format!("/admin/supplies?provider_id={business}");

    let all = page(&e, "/admin/supplies", &admin).await;
    assert!(
        !all.contains(r#"id="them-vat-tu""#),
        "create needs a provider"
    );
    assert!(page(&e, &list, &admin)
        .await
        .contains(r#"id="them-vat-tu""#));

    // The owner's rules: refused in the sheet, open again, typed values kept.
    let business_id = business.to_string();
    let category = category.to_string();
    let create = |name: &'static str| {
        post_form(
            "/admin/supplies",
            &[
                ("business_id", business_id.as_str()),
                ("category_id", category.as_str()),
                ("name", name),
                ("price", "350.000"),
                ("unit", "bao"),
            ],
            Some(&admin),
        )
    };
    let res = send(&e.router, create(" ")).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains(r#"id="them-vat-tu""#));
    assert!(html.contains(r#"value="350.000""#), "typed price kept");

    let res = send(&e.router, create("Phân NPK 20-20-15")).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let own = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    let id = own.rsplit('/').next().unwrap().to_owned();
    assert_eq!(
        one_i32(
            &e,
            &format!("SELECT business_id FROM products WHERE id = {id}")
        )
        .await,
        business
    );

    // A photo with the boxes, as on the owner's form.
    let res = send(
        &e.router,
        post_multipart_form(
            &own,
            &[
                ("category_id", category.as_str()),
                ("name", "Phân NPK 20-20-15"),
            ],
            Some(&photo(64, 48)),
            &admin,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let has_photo =
        format!("SELECT count(*)::int FROM products WHERE id = {id} AND image_file_id IS NOT NULL");
    assert_eq!(one_i32(&e, &has_photo).await, 1);
    assert!(page(&e, &own, &admin).await.contains(r#"id="go-anh""#));

    // A file that is not an image is refused on the photo box.
    let res = send(
        &e.router,
        post_multipart_form(
            &own,
            &[("category_id", category.as_str()), ("name", "Phân NPK")],
            Some(b"not an image"),
            &admin,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains(r#"id="edit-image-error""#));
    assert_eq!(
        one_text(&e, &format!("SELECT name FROM products WHERE id = {id}")).await,
        "Phân NPK 20-20-15",
        "nothing saved with a refused photo"
    );

    // Taken off behind a confirmation with a reason: the supply stays.
    let res = send(
        &e.router,
        post_form(
            &format!("{own}/image/delete"),
            &[("reason", "Ảnh không phải vật tư")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], own.as_str());
    assert_eq!(one_i32(&e, &has_photo).await, 0);
    assert_eq!(
        one_text(
            &e,
            "SELECT action || ': ' || reason FROM audit_logs WHERE entity_type = 'product' ORDER BY id DESC LIMIT 1"
        )
        .await,
        "remove_product_image: Ảnh không phải vật tư"
    );
    let html = page(&e, &own, &admin).await;
    assert!(!html.contains(r#"id="go-anh""#));
    assert!(
        html.contains("Ảnh không phải vật tư"),
        "the reason in its history"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_shown_service_says_why_farmers_cannot_see_it() {
    let e = env_with_origin().await;
    let (admin, _, business) = setup(&e).await;
    let (category, _) = categories_of(&e, "service").await[0].clone();
    let res = send(
        &e.router,
        post_form(
            "/admin/services",
            &[
                ("business_id", &business.to_string()),
                ("category_id", &category.to_string()),
                ("name", "Phun thuốc"),
            ],
            Some(&admin),
        ),
    )
    .await;
    let own = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    let id = own.rsplit('/').next().unwrap().to_owned();
    let public = format!(r#"href="/services/{id}/"#);

    let html = page(&e, &own, &admin).await;
    assert!(html.contains("Đang hiện ở trang công khai."));
    assert!(html.contains(&public), "its public page while public");

    // Its provider switched off: the service's own switch is still on, but
    // the page says it is not public, why, and where to fix it.
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE businesses SET is_active = false WHERE id = {business}"
        ))
        .await
        .unwrap();
    let html = page(&e, &own, &admin).await;
    assert!(html.contains("Đang bật hiện, nhưng bà con chưa thấy."));
    assert!(html.contains("Nhà cung cấp đang tắt."));
    assert!(html.contains(&format!(r#"href="/admin/providers/{business}""#)));
    assert!(!html.contains(&public), "no link to a page that would 404");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_supply_page_keeps_its_list_and_says_who_listed_it() {
    let e = env_with_origin().await;
    let (admin, token, business) = setup(&e).await;
    let (category, _) = categories_of(&e, "product").await[0].clone();
    let id = create_product(
        &e,
        &token,
        &format!(r#"{{"category_id":{category},"name":"Phân bón lá"}}"#),
    )
    .await;

    // Opened from a page of a searched list, the page leads back to it.
    let list = page(&e, "/admin/supplies?q=bon", &admin).await;
    let own = format!("/admin/supplies/{id}?q=bon");
    assert!(
        list.contains(&format!(r#"href="{own}""#)),
        "the row keeps the list"
    );
    let html = page(&e, &own, &admin).await;
    assert!(
        html.contains(r#"href="/admin/supplies?q=bon""#),
        "back to that list"
    );
    assert!(html.contains("Nhà cung cấp tự thêm"), "the owner listed it");
    assert!(html.contains("Nhà cung cấp tự đăng mục này"));
    assert!(html.contains("Ngành:"));
    assert!(html.contains("0 lượt gọi, Zalo trong 30 ngày"));
    assert!(html.contains("0 yêu cầu liên hệ"));

    // Saving lands on the page again, the list still behind it.
    let category = category.to_string();
    let res = send(
        &e.router,
        post_multipart_form(
            &own,
            &[
                ("category_id", category.as_str()),
                ("name", "Phân bón lá NPK"),
            ],
            None,
            &admin,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], own.as_str());

    // Opened with no list, it leads to the provider's supplies.
    let html = page(&e, &format!("/admin/supplies/{id}"), &admin).await;
    assert!(html.contains(&format!(r#"href="/admin/supplies?provider_id={business}""#)));
    e.cleanup().await;
}
