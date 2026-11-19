//! The ways between pages: a detail page back to its category, a public
//! record to where its owner or an admin edits it, and the admin pages of a
//! record and of the account that manages it, each to the other.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use infrastructure::entities::{business, user};
use integration_tests::{
    body_text, categories_of, create_product, env_with_origin, get_page, install_admin, json_body,
    register_provider, send, sign_in, with_product_category, Env,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// A provider "nongdan" with one product and one service; returns
/// (business id, user id, product id, service id).
async fn seeded(e: &Env) -> (i32, i32, i32, i32) {
    let token = register_provider(&e.state, "nongdan", "0901234567").await;
    let product = create_product(
        e,
        &token,
        &with_product_category(e, r#"{"name":"Phân bón NPK","price":15000,"unit":"kg"}"#).await,
    )
    .await;
    let category = categories_of(e, "service").await[0].0;
    let req = Request::post("/api/provider/services")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!(
            r#"{{"category_id":{category},"name":"Múc bùn"}}"#
        )))
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let service = json_body(res).await["id"].as_i64().unwrap() as i32;
    let business = business::Entity::find()
        .filter(business::Column::Name.eq("Vườn nongdan"))
        .one(&e.t.db)
        .await
        .unwrap()
        .unwrap()
        .id;
    let account = user::Entity::find()
        .filter(user::Column::Username.eq("nongdan"))
        .one(&e.t.db)
        .await
        .unwrap()
        .unwrap()
        .id;
    (business, account, product, service)
}

/// The page at `path` for `cookies`, following the canonical redirect.
async fn page(e: &Env, path: &str, cookies: Option<&str>) -> String {
    let res = send(&e.router, get_page(path, cookies)).await;
    if res.status() == StatusCode::MOVED_PERMANENTLY {
        let to = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
        return body_text(send(&e.router, get_page(&to, cookies)).await).await;
    }
    assert_eq!(res.status(), StatusCode::OK, "{path}");
    body_text(res).await
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_public_record_leads_its_owner_and_an_admin_to_where_it_is_edited() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let (business, _, product, service) = seeded(&e).await;
    register_provider(&e.state, "hangxom", "0907654321").await;
    let owner = sign_in(&e, "nongdan", "matkhau123").await;
    let neighbour = sign_in(&e, "hangxom", "matkhau123").await;

    let cases = [
        (
            format!("/supplies/{product}"),
            format!("/provider/supplies/{product}/edit"),
            format!("/admin/supplies/{product}"),
        ),
        (
            format!("/services/{service}"),
            format!("/provider/services/{service}/edit"),
            format!("/admin/services/{service}"),
        ),
        (
            format!("/providers/{business}"),
            "/provider/dashboard".to_owned(),
            format!("/admin/providers/{business}"),
        ),
    ];
    for (public, own_edit, admin_edit) in cases {
        let own = format!(r#"href="{own_edit}""#);
        let panel = format!(r#"href="{admin_edit}""#);
        let anonymous = page(&e, &public, None).await;
        assert!(
            !anonymous.contains(&own) && !anonymous.contains(&panel),
            "{public}"
        );
        assert!(
            page(&e, &public, Some(&owner)).await.contains(&own),
            "{public}"
        );
        // Every provider has the dashboard in its tabs: the edit button is
        // told apart by its words.
        let other = page(&e, &public, Some(&neighbour)).await;
        assert!(
            !other.contains("Sửa thông tin của bạn"),
            "not someone else's: {public}"
        );
        assert!(
            page(&e, &public, Some(&admin)).await.contains(&panel),
            "{public}"
        );
    }
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_detail_page_goes_back_to_its_category_and_lists_offer_near_me() {
    let e = env_with_origin().await;
    install_admin(&e).await;
    let (_, _, product, service) = seeded(&e).await;

    let back = |page: &str| {
        page.split(r#"class="back-link" href=""#)
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .map(str::to_owned)
    };
    let product_page = page(&e, &format!("/supplies/{product}"), None).await;
    let to = back(&product_page).expect("a back link");
    assert!(to.starts_with("/supplies/category/"), "{to}");
    let service_page = page(&e, &format!("/services/{service}"), None).await;
    let to = back(&service_page).expect("a back link");
    assert!(to.starts_with("/services/category/"), "{to}");

    let list = page(&e, "/supplies", None).await;
    assert!(
        list.contains(r#"href="/explore?tab=products""#),
        "the near-me banner, as on /services"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn admin_pages_link_a_record_and_its_account_both_ways() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let (business, account, product, service) = seeded(&e).await;
    let res = send(
        &e.router,
        integration_tests::post_form(
            &format!("/admin/providers/{business}/account"),
            &[("username", "nongdan")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "assign the account");

    let detail = page(&e, &format!("/admin/providers/{business}"), Some(&admin)).await;
    assert!(
        detail.contains(&format!(r#"href="/admin/users/{account}""#)),
        "the managing account opens"
    );
    let detail = page(
        &e,
        &format!("/admin/providers/{business}?tab=offers"),
        Some(&admin),
    )
    .await;
    assert!(
        detail.contains(&format!(r#"href="/admin/supplies/{product}""#)),
        "products open in the panel, like services"
    );

    let user = page(&e, &format!("/admin/users/{account}"), Some(&admin)).await;
    assert!(
        user.contains(&format!(r#"href="/admin/providers/{business}""#)),
        "the account leads to its business"
    );

    let svc = page(&e, &format!("/admin/services/{service}"), Some(&admin)).await;
    assert!(
        svc.contains(&format!(r#"href="/services/{service}/"#)),
        "the public page, as on a product's"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_former_admin_provider_addresses_move_for_good() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    for (old, new) in [
        ("/admin/explore", "/admin/providers"),
        (
            "/admin/explore?page=2&q=bun",
            "/admin/providers?page=2&q=bun",
        ),
        ("/admin/explore/7", "/admin/providers/7"),
    ] {
        let res = send(&e.router, get_page(old, Some(&admin))).await;
        assert_eq!(res.status(), StatusCode::MOVED_PERMANENTLY, "{old}");
        assert_eq!(res.headers()[header::LOCATION], new);
    }
    e.cleanup().await;
}
