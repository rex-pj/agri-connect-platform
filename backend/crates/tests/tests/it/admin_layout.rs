//! The admin panel arranged by what goes together: the price board is one
//! menu entry with two tabs (the prices entered, the produce followed), each
//! price row leads to its crop and area, a crop's page shows its prices, a
//! commodity's page its crops and the supplies for it, roles and their
//! permissions share a page, and the home page's two rows of tiles sit side
//! by side on a wide screen.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, env_with_origin, get_page, install_admin, post_form, send, Env,
};
use sea_orm::{ConnectionTrait, Statement};

async fn scalar(e: &Env, sql: &str) -> String {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            sql.to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "v")
        .unwrap()
}

async fn page(e: &Env, path: &str, cookie: &str) -> String {
    let res = send(&e.router, get_page(path, Some(cookie))).await;
    assert_eq!(res.status(), StatusCode::OK, "{path}");
    body_text(res).await
}

async fn moved_to(e: &Env, from: &str, cookie: &str) -> String {
    let res = send(&e.router, get_page(from, Some(cookie))).await;
    assert_eq!(res.status(), StatusCode::MOVED_PERMANENTLY, "{from}");
    res.headers()[header::LOCATION].to_str().unwrap().to_owned()
}

/// An admin with the demo data loaded from the demo data page.
async fn with_demo(e: &Env) -> String {
    let cookie = install_admin(e).await;
    let res = send(
        &e.router,
        post_form("/setup/seed", &[("action", "load")], Some(&cookie)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    cookie
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_price_board_is_one_entry_with_its_produce_on_a_tab() {
    let e = env_with_origin().await;
    let admin = with_demo(&e).await;
    let st25 = scalar(
        &e,
        "SELECT id::text AS v FROM crops WHERE name = 'Lúa ST25'",
    )
    .await;

    // Each price leads to its crop and its area.
    let prices = page(&e, "/admin/prices", &admin).await;
    assert!(prices.contains(&format!(r#"href="/admin/crops/{st25}""#)));
    assert!(prices.contains(r#"href="/admin/locations/"#));
    assert!(prices.contains(r#"href="/admin/prices?tab=crops""#));
    assert!(
        !prices.contains(r#"href="/admin/crops""#),
        "no menu entry of its own"
    );

    // Narrowed to one crop: only its prices, and the way back.
    let one = page(&e, &format!("/admin/prices?crop_id={st25}"), &admin).await;
    assert!(one.contains("admin-filter"));
    assert_eq!(
        one.matches(&format!(r#"href="/admin/crops/{st25}""#))
            .count(),
        3,
        "the filter bar and its two prices"
    );

    // The produce is the second tab; the former list moves there.
    let crops = page(&e, "/admin/prices?tab=crops", &admin).await;
    assert!(crops.contains("Lúa ST25") && crops.contains("Heo hơi"));
    assert!(crops.contains(r#"href="/admin/prices?tab=crops" aria-current="page""#));
    assert_eq!(
        moved_to(&e, "/admin/crops?hide=3", &admin).await,
        "/admin/prices?tab=crops&hide=3"
    );

    // A crop's page: its prices, the price sheet with it chosen, its commodity.
    let crop = page(&e, &format!("/admin/crops/{st25}"), &admin).await;
    assert!(crop.contains("Cai Lậy, Đồng Tháp"));
    assert!(crop.contains(&format!("/admin/prices?crop_id={st25}#nhap-gia")));
    assert!(crop.contains("Thuộc Lúa"));
    let sheet = page(&e, &format!("/admin/prices?crop_id={st25}"), &admin).await;
    assert!(sheet.contains(&format!(r#"<option value="{st25}" selected>"#)));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_commodity_page_gathers_its_crops_and_supplies() {
    let e = env_with_origin().await;
    let admin = with_demo(&e).await;
    let rice = scalar(
        &e,
        "SELECT id::text AS v FROM commodities WHERE name = 'Lúa'",
    )
    .await;

    let html = page(&e, &format!("/admin/commodities/{rice}"), &admin).await;
    assert!(html.contains("Lúa ST25") && html.contains("Lúa OM 18"));
    assert!(!html.contains("Bò hơi"));
    assert!(html.contains(&format!("/admin/supplies?commodity_id={rice}")));
    assert!(html.contains("4 vật tư dùng cho"));

    // Its add-a-crop sheet has the commodity chosen.
    assert!(html.contains(&format!(r#"<option value="{rice}" selected>"#)));
    let res = send(
        &e.router,
        post_form(
            "/admin/crops",
            &[
                ("name", "Lúa Nàng Hoa 9"),
                ("commodity_id", &rice),
                ("unit", "kg"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let html = page(&e, &format!("/admin/commodities/{rice}"), &admin).await;
    assert!(html.contains("Lúa Nàng Hoa 9"));

    // The supply list narrowed to it, then to one supply type.
    let supplies = page(&e, &format!("/admin/supplies?commodity_id={rice}"), &admin).await;
    assert!(supplies.contains("Lúa giống ST25 cấp xác nhận"));
    assert!(!supplies.contains("Phân NPK"), "only those for rice");
    assert!(!supplies.contains("Cám heo"));
    let seed = scalar(
        &e,
        "SELECT id::text AS v FROM supply_types WHERE slug = 'giong'",
    )
    .await;
    let typed = page(&e, &format!("/admin/categories/{seed}"), &admin).await;
    assert!(typed.contains(&format!("/admin/supplies?category_id={seed}")));
    let of_type = page(&e, &format!("/admin/supplies?category_id={seed}"), &admin).await;
    assert!(of_type.contains("Tôm thẻ giống PL12") && !of_type.contains("Phân NPK"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn roles_and_permissions_share_one_page() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    assert_eq!(
        moved_to(&e, "/admin/permissions", &admin).await,
        "/admin/roles?tab=permissions"
    );
    let grid = page(&e, "/admin/roles?tab=permissions", &admin).await;
    assert!(grid.contains("perm-matrix"));
    assert!(grid.contains(r#"href="/admin/roles?tab=permissions" aria-current="page""#));
    let roles = page(&e, "/admin/roles", &admin).await;
    assert!(roles.contains(r#"href="/admin/roles" aria-current="page""#));
    assert!(
        !roles.contains(r#"href="/admin/permissions""#),
        "no menu entry of its own"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_home_page_puts_each_errand_in_a_section_of_its_own() {
    let e = env_with_origin().await;
    with_demo(&e).await;
    let res = send(&e.router, get_page("/", None)).await;
    let home = body_text(res).await;
    assert_eq!(home.matches(r#"<section class="home-errand">"#).count(), 2);
    assert!(
        home.contains(r#"title="Nạo vét""#),
        "the whole name on hover"
    );
    // Each row is one line until opened: the rest wait behind a "+N"
    // toggle (a label for a checkbox, so without JS too), and the header
    // leads to the whole list.
    assert_eq!(home.matches("Xem tất cả").count(), 2);
    let toggles = home.matches(r#"class="cat-fold visually-hidden""#).count();
    assert!(toggles >= 1, "the demo catalogue overflows a row");
    assert_eq!(home.matches(r#"<label for="more-"#).count(), toggles);
    assert!(home.contains("cat-fold--all") || home.contains("cat-fold--phone"));
    e.cleanup().await;
}
