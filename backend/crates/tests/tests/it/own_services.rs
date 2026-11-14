//! A provider's own services: /provider/services/* pages and
//! /api/provider/services. Ownership, the admin's hide that an edit never
//! undoes, revoked permissions, the per-business cap, the optional photo and
//! its cleanup. Database tests need TEST_DATABASE_URL and `--include-ignored`.

use application::authz::Actor;
use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use infrastructure::entities::{audit_log, business, file, role, service};
use integration_tests::{
    body_text, categories_of, env_with_origin, first_admin, get_page, json_body, multipart, photo,
    post_form, post_multipart_form, register_provider, send, sign_in, Env, MULTIPART_BOUNDARY,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

const NEW: &str = "/provider/services/new";

async fn page(e: &Env, path: &str, cookie: &str) -> String {
    body_text(send(&e.router, get_page(path, Some(cookie))).await).await
}

/// The provider "nongdan": their API token, their session cookie, and the
/// first service category (id, name).
async fn provider(e: &Env) -> (String, String, (i32, String)) {
    let token = register_provider(&e.state, "nongdan", "0901234567").await;
    let cookie = sign_in(e, "nongdan", "matkhau123").await;
    let (id, _) = categories_of(e, "service").await[0].clone();
    let name = category_name(e, id).await;
    (token, cookie, (id, name))
}

async fn category_name(e: &Env, id: i32) -> String {
    infrastructure::entities::service_type::Entity::find_by_id(id)
        .one(&e.t.db)
        .await
        .unwrap()
        .unwrap()
        .name
}

/// Live (not soft-deleted) services.
async fn live_services(e: &Env) -> Vec<service::Model> {
    service::Entity::find()
        .filter(service::Column::DeletedAt.is_null())
        .order_by_asc(service::Column::Id)
        .all(&e.t.db)
        .await
        .unwrap()
}

async fn files(e: &Env) -> u64 {
    file::Entity::find().count(&e.t.db).await.unwrap()
}

async fn audited(e: &Env, action: &str) -> u64 {
    audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq(action))
        .count(&e.t.db)
        .await
        .unwrap()
}

/// Takes `key` away from the provider role, through the same use case as the
/// admin's permission grid.
async fn revoke(e: &Env, admin: &Actor, key: &str) {
    let provider_role = role::Entity::find()
        .filter(role::Column::Code.eq("provider"))
        .one(&e.t.db)
        .await
        .unwrap()
        .unwrap();
    let permission = &e.state.usecases.permission;
    let grants = permission
        .role_grants(admin, provider_role.id)
        .await
        .unwrap();
    let keep: Vec<String> = grants
        .permissions
        .into_iter()
        .filter(|g| g.granted && g.code != key)
        .map(|g| g.code)
        .collect();
    permission
        .set_role_grants(admin, provider_role.id, keep, None)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_provider_role_cannot_be_given_an_admins_permission() {
    let e = env_with_origin().await;
    let admin = first_admin(&e.state).await;
    let provider_role = role::Entity::find()
        .filter(role::Column::Code.eq("provider"))
        .one(&e.t.db)
        .await
        .unwrap()
        .unwrap();
    let permission = &e.state.usecases.permission;
    let grants = permission
        .role_grants(&admin, provider_role.id)
        .await
        .unwrap();
    assert!(grants
        .permissions
        .iter()
        .all(|g| g.grantable == g.code.starts_with("own.")));
    let mut keys: Vec<String> = grants
        .permissions
        .into_iter()
        .filter(|g| g.granted)
        .map(|g| g.code)
        .collect();
    let before = keys.clone();
    keys.push("user.read".into());
    let refused = permission
        .set_role_grants(&admin, provider_role.id, keys, None)
        .await;
    assert_eq!(
        refused.err().map(|e| e.code()),
        Some(domain::ErrorCode::PermissionNotForRole)
    );
    let after: Vec<String> = permission
        .role_grants(&admin, provider_role.id)
        .await
        .unwrap()
        .permissions
        .into_iter()
        .filter(|g| g.granted)
        .map(|g| g.code)
        .collect();
    assert_eq!(after, before, "nothing was granted");
    e.cleanup().await;
}

fn api(method: Method, path: &str, token: &str, json: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json.to_owned()))
        .unwrap()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_service_is_added_with_its_photo_edited_and_deleted() {
    let e = env_with_origin().await;
    let (_, cookie, (category, category_name)) = provider(&e).await;
    let category = category.to_string();

    // No service yet: the dashboard leads with one large button.
    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(html.contains("Chưa có dịch vụ"));
    assert!(html.contains(r#"href="/provider/services/new""#));
    let html = page(&e, NEW, &cookie).await;
    assert!(html.contains(r#"enctype="multipart/form-data""#));
    assert!(html.contains(r#"class="choice-card""#), "category cards");
    assert!(!html.contains("capture="), "the gallery can be used too");
    assert!(
        html.contains("photo-picker__tile"),
        "the photo tile is the button"
    );
    assert!(html.contains("Chọn ảnh"), "empty: it says what to do");

    // No category: refused on that field, nothing stored.
    let res = send(
        &e.router,
        post_multipart_form(
            NEW,
            &[("price_note", "Từ 500.000đ")],
            Some(&photo(64, 48)),
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains(r#"id="sv-category-error""#));
    assert!(html.contains(r#"value="Từ 500.000đ""#), "typed price kept");
    assert_eq!(files(&e).await, 0, "a refused form stores no photo");

    // A blank name takes the category's; the photo goes in with the row.
    let res = send(
        &e.router,
        post_multipart_form(
            NEW,
            &[
                ("category_id", &category),
                ("name", " "),
                ("price_note", "Liên hệ"),
            ],
            Some(&photo(64, 48)),
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/provider/dashboard");
    let services = live_services(&e).await;
    assert_eq!(services.len(), 1);
    let created = &services[0];
    assert_eq!(created.name, category_name);
    let first_photo = created.image_file_id.expect("photo attached");
    assert_eq!(files(&e).await, 1);
    assert_eq!(audited(&e, "create_service").await, 1);

    // Farmers see it with its photo; the home page waits for verification.
    let biz = business::Entity::find_by_id(created.business_id)
        .one(&e.t.db)
        .await
        .unwrap()
        .unwrap();
    let res = send(&e.router, get_page(&format!("/providers/{}", biz.id), None)).await;
    let canonical = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    let public = body_text(send(&e.router, get_page(&canonical, None)).await).await;
    assert!(public.contains(&format!("/files/{first_photo}/thumb")));
    assert!(public.contains(r#"class="service-list__photo""#));
    let featured = e
        .state
        .usecases
        .explore
        .get_explore_data(None)
        .await
        .unwrap();
    assert!(
        featured.featured_providers.is_empty(),
        "an unverified account is not featured"
    );
    let admin = first_admin(&e.state).await;
    e.state
        .usecases
        .admin
        .set_verification(&admin, biz.id, true, None)
        .await
        .unwrap();
    let featured = e
        .state
        .usecases
        .explore
        .get_explore_data(None)
        .await
        .unwrap();
    assert_eq!(featured.featured_providers.len(), 1, "verified: featured");

    // Edit with a new photo: the old one is released.
    let edit = format!("/provider/services/{}/edit", created.id);
    let html = page(&e, &edit, &cookie).await;
    assert!(html.contains(&format!(r#"value="{category_name}""#)));
    assert!(html.contains("Ảnh hiện tại"));
    assert!(html.contains("Đổi ảnh"), "with a photo: how to change it");
    let res = send(
        &e.router,
        post_multipart_form(
            &edit,
            &[
                ("category_id", &category),
                ("name", "Múc bùn vườn sầu riêng"),
            ],
            Some(&photo(32, 32)),
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let edited = &live_services(&e).await[0];
    assert_eq!(edited.name, "Múc bùn vườn sầu riêng");
    assert_ne!(edited.image_file_id, Some(first_photo));
    assert_eq!(files(&e).await, 1, "the replaced photo is gone");
    assert_eq!(audited(&e, "update_service").await, 1);

    // Delete: the photo goes with it, and the page is gone.
    let res = send(
        &e.router,
        post_form(
            &format!("/provider/services/{}/delete", created.id),
            &[],
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(live_services(&e).await.is_empty());
    assert_eq!(files(&e).await, 0);
    let res = send(&e.router, get_page(&edit, Some(&cookie))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn another_providers_service_looks_missing() {
    let e = env_with_origin().await;
    let (token, _, (category, _)) = provider(&e).await;
    let res = send(
        &e.router,
        api(
            Method::POST,
            "/api/provider/services",
            &token,
            &format!(r#"{{"category_id":{category},"name":"Phun thuốc drone"}}"#),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let id = json_body(res).await["id"].as_i64().unwrap();

    let other_token = register_provider(&e.state, "hangxom", "0907654321").await;
    let other = sign_in(&e, "hangxom", "matkhau123").await;
    let edit = format!("/provider/services/{id}/edit");
    let res = send(&e.router, get_page(&edit, Some(&other))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let res = send(
        &e.router,
        post_multipart_form(
            &edit,
            &[("category_id", &category.to_string()), ("name", "Của tôi")],
            Some(&photo(32, 32)),
            &other,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(files(&e).await, 0, "never decoded");
    let res = send(
        &e.router,
        post_form(
            &format!("/provider/services/{id}/delete"),
            &[],
            Some(&other),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // The API agrees, the photo endpoint included.
    let path = format!("/api/provider/services/{id}");
    let body = format!(r#"{{"category_id":{category},"name":"Của tôi"}}"#);
    let res = send(&e.router, api(Method::PUT, &path, &other_token, &body)).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_body(res).await["code"], "service_not_found");
    let res = send(&e.router, api(Method::DELETE, &path, &other_token, "")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let upload = Request::builder()
        .method(Method::PUT)
        .uri(format!("{path}/image"))
        .header(header::AUTHORIZATION, format!("Bearer {other_token}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}"),
        )
        .body(Body::from(multipart("image", &photo(32, 32))))
        .unwrap();
    let res = send(&e.router, upload).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(files(&e).await, 0);
    let res = send(
        &e.router,
        api(Method::GET, "/api/provider/services", &other_token, ""),
    )
    .await;
    assert_eq!(json_body(res).await, serde_json::json!([]));

    assert_eq!(live_services(&e).await[0].name, "Phun thuốc drone");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_service_an_admin_hid_stays_hidden_when_its_owner_edits_it() {
    let e = env_with_origin().await;
    let (token, cookie, (category, _)) = provider(&e).await;
    let res = send(
        &e.router,
        api(
            Method::POST,
            "/api/provider/services",
            &token,
            &format!(r#"{{"category_id":{category},"name":"Sửa máy cày"}}"#),
        ),
    )
    .await;
    let id = json_body(res).await["id"].as_i64().unwrap() as i32;
    let admin = first_admin(&e.state).await;
    e.state
        .usecases
        .admin
        .toggle_service_active(&admin, id, false, Some("Sai danh mục".into()))
        .await
        .unwrap();

    let res = send(
        &e.router,
        api(
            Method::PUT,
            &format!("/api/provider/services/{id}"),
            &token,
            &format!(r#"{{"category_id":{category},"name":"Sửa máy cày (mới)"}}"#),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["name"], "Sửa máy cày (mới)");
    assert_eq!(body["is_active"], false, "still hidden");

    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(html.contains("Quản trị viên đã ẩn"));
    let html = page(&e, &format!("/provider/services/{id}/edit"), &cookie).await;
    assert!(html.contains("Quản trị viên đã ẩn dịch vụ này"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn revoked_permissions_hide_the_buttons_and_refuse_the_writes() {
    let e = env_with_origin().await;
    let (token, cookie, (category, _)) = provider(&e).await;
    let res = send(
        &e.router,
        api(
            Method::POST,
            "/api/provider/services",
            &token,
            &format!(r#"{{"category_id":{category}}}"#),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED, "name defaults");
    let id = json_body(res).await["id"].as_i64().unwrap();
    let admin = first_admin(&e.state).await;

    revoke(&e, &admin, "own.service.create").await;
    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(!html.contains(r#"href="/provider/services/new""#));
    let res = send(
        &e.router,
        post_multipart_form(
            NEW,
            &[("category_id", &category.to_string())],
            None,
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    revoke(&e, &admin, "own.service.update").await;
    let edit = format!("/provider/services/{id}/edit");
    let html = page(&e, &edit, &cookie).await;
    assert!(html.contains("chưa được phép sửa dịch vụ"));
    assert!(!html.contains(r#"type="submit" class="btn btn-success btn-lg w-100""#));
    let res = send(
        &e.router,
        post_multipart_form(
            &edit,
            &[("category_id", &category.to_string()), ("name", "Khác")],
            None,
            &cookie,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    revoke(&e, &admin, "own.service.delete").await;
    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(
        !html.contains(&format!(r#"href="{edit}""#)),
        "nothing left to do with it: no edit button"
    );
    let res = send(
        &e.router,
        api(
            Method::DELETE,
            &format!("/api/provider/services/{id}"),
            &token,
            "",
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(live_services(&e).await.len(), 1);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_rules_hold_through_the_api_and_a_business_lists_at_most_the_cap() {
    let e = env_with_origin().await;
    let (token, cookie, (category, _)) = provider(&e).await;
    let (product_category, _) = categories_of(&e, "product").await[0].clone();

    let res = send(
        &e.router,
        api(
            Method::POST,
            "/api/provider/services",
            &token,
            &format!(r#"{{"category_id":{product_category},"name":"Gạo"}}"#),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(res).await["field_errors"]["category_id"][0],
        "Vui lòng chọn loại dịch vụ."
    );

    let max = application::constants::SERVICES_PER_BUSINESS_MAX;
    for n in 0..max {
        let res = send(
            &e.router,
            api(
                Method::POST,
                "/api/provider/services",
                &token,
                &format!(r#"{{"category_id":{category},"name":"Dịch vụ {n}"}}"#),
            ),
        )
        .await;
        assert_eq!(res.status(), StatusCode::CREATED);
    }
    let res = send(
        &e.router,
        api(
            Method::POST,
            "/api/provider/services",
            &token,
            &format!(r#"{{"category_id":{category},"name":"Một nữa"}}"#),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(json_body(res).await["code"], "service_limit_reached");
    assert_eq!(live_services(&e).await.len() as u64, max);

    let html = page(&e, "/provider/dashboard", &cookie).await;
    assert!(html.contains(&format!("Đã đủ {max} dịch vụ")));
    assert!(!html.contains(r#"href="/provider/services/new""#));
    e.cleanup().await;
}
