//! The service listing (/services, category pages), a service's own page
//! (/services/{id}/{slug}) with its contact form, and the provider directory
//! (/providers): GET filters, sorts, canonical URLs, and which services are
//! public. Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::{Duration, Utc};
use infrastructure::entities::{business, inquiry, service, service_type as category};
use integration_tests::{
    body_text, categories_of, env_with_origin, json_body, post_form, send, Env,
};
use sea_orm::{sea_query::Expr, ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

fn page(uri: &str) -> Request<Body> {
    Request::get(uri)
        .header(header::ACCEPT, "text/html")
        .body(Body::empty())
        .unwrap()
}

async fn html(e: &Env, uri: &str) -> String {
    let res = send(&e.router, page(uri)).await;
    assert_eq!(res.status(), StatusCode::OK, "{uri}");
    body_text(res).await
}

/// Provider names in page order (provider cards).
fn provider_names(html: &str) -> Vec<String> {
    html.split(r#"<h3 class="card-title fs-6 fw-bold mb-0 flex-grow-1">"#)
        .skip(1)
        .filter_map(|s| s.split_once("</a></h3>"))
        .filter_map(|(link, _)| link.rsplit_once('>').map(|(_, name)| name.to_owned()))
        .collect()
}

/// Service names in page order (service cards).
fn service_names(html: &str) -> Vec<String> {
    html.split(r#"<article class="card h-100 product-card service-card"#)
        .skip(1)
        .filter_map(|card| {
            let (_, title) = card.split_once("product-card__name")?;
            let (_, rest) = title.split_once(r#"">"#)?;
            let (_, name) = rest.split_once(r#"">"#)?;
            name.split_once("</a>").map(|(n, _)| n.to_owned())
        })
        .collect()
}

async fn add_business(
    e: &Env,
    name: &str,
    phone: &str,
    address: Option<&str>,
    verified: bool,
    days_ago: i64,
) -> i32 {
    business::ActiveModel {
        name: Set(name.to_owned()),
        phone: Set(phone.to_owned()),
        address: Set(address.map(str::to_owned)),
        is_verified: Set(verified),
        created_on: Set((Utc::now() - Duration::days(days_ago)).fixed_offset()),
        ..Default::default()
    }
    .insert(&e.t.db)
    .await
    .unwrap()
    .id
}

async fn add_service(
    e: &Env,
    business_id: i32,
    category_id: i32,
    name: &str,
    price_note: Option<&str>,
    days_ago: i64,
) -> i32 {
    service::ActiveModel {
        business_id: Set(business_id),
        service_type_id: Set(category_id),
        name: Set(name.to_owned()),
        price_note: Set(price_note.map(str::to_owned)),
        created_on: Set((Utc::now() - Duration::days(days_ago)).fixed_offset()),
        ..Default::default()
    }
    .insert(&e.t.db)
    .await
    .unwrap()
    .id
}

/// Ids of what `seeded` made.
struct Seed {
    /// The two service categories (id, slug).
    a: (i32, String),
    b: (i32, String),
    /// "Sửa máy cày" of Cơ khí Bảy Tốt, who also offers "Sửa máy bơm".
    repair: i32,
    bay_tot: i32,
}

/// Four providers: Cơ khí Bảy Tốt (two services, one in each category),
/// Múc bùn Út Hiền (verified, one in the first), Drone phun Ba Lúa (one in
/// the second), and a new sign-up with no service yet.
async fn seeded(e: &Env) -> Seed {
    let cats = categories_of(e, "service").await;
    let (a, b) = (cats[0].clone(), cats[1].clone());
    let bay_tot = add_business(
        e,
        "Cơ khí Bảy Tốt",
        "0901000001",
        Some("Khu phố 1, phường Cai Lậy, Đồng Tháp"),
        false,
        3,
    )
    .await;
    let ut_hien = add_business(
        e,
        "Múc bùn Út Hiền",
        "0901000002",
        Some("Long Xuyên, An Giang"),
        true,
        2,
    )
    .await;
    let ba_lua = add_business(e, "Drone phun Ba Lúa", "0901000003", None, false, 1).await;
    add_business(e, "Vườn mới đăng ký", "0901000004", None, false, 4).await;
    let repair = add_service(e, bay_tot, a.0, "Sửa máy cày", Some("300000"), 4).await;
    add_service(e, bay_tot, b.0, "Sửa máy bơm", None, 3).await;
    add_service(e, ut_hien, a.0, "Múc bùn ao", Some("Từ 500.000đ/công"), 2).await;
    add_service(e, ba_lua, b.0, "Phun thuốc drone", None, 1).await;
    Seed {
        a,
        b,
        repair,
        bay_tot,
    }
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_listing_shows_services_with_their_provider_and_filters_them() {
    let e = Env::new().await;
    let seed = seeded(&e).await;

    let all = html(&e, "/services").await;
    assert!(all.contains("<title>Dịch vụ nông nghiệp | Agri-Connect</title>"));
    assert!(
        all.contains(r#"<meta name="description" content="Thuê múc bùn, phun thuốc bằng drone"#)
    );
    assert!(all.contains("<strong>4</strong> dịch vụ"));
    // Default order: the verified provider's first, then the newest.
    assert_eq!(
        service_names(&all),
        [
            "Múc bùn ao",
            "Phun thuốc drone",
            "Sửa máy bơm",
            "Sửa máy cày"
        ]
    );
    assert!(
        !all.contains("Vườn mới đăng ký"),
        "no service, not in this list"
    );
    assert!(all.contains("Cơ khí Bảy Tốt"), "the provider on the card");
    assert!(all.contains(r#"href="tel:0901000001""#), "call button");
    assert!(all.contains("300.000 ₫"), "a bare amount reads as đồng");
    assert!(all.contains("Từ 500.000đ/công"), "a worded price as typed");
    assert!(all.contains(&format!(r#"href="/services/{}/sua-may-cay""#, seed.repair)));
    assert!(
        all.contains(r#"href="/providers""#),
        "the directory is one tap away"
    );

    let sorted = html(&e, "/services?sort=name_asc").await;
    assert_eq!(
        service_names(&sorted),
        [
            "Múc bùn ao",
            "Phun thuốc drone",
            "Sửa máy bơm",
            "Sửa máy cày"
        ]
    );
    // The typed text matches the service and the provider's name.
    assert_eq!(
        service_names(&html(&e, "/services?q=phun").await),
        ["Phun thuốc drone"]
    );
    assert_eq!(
        service_names(&html(&e, "/services?q=bay+tot&sort=name_asc").await),
        ["Sửa máy bơm", "Sửa máy cày"]
    );
    let in_category = html(&e, &format!("/services?service_id={}", seed.a.0)).await;
    assert_eq!(service_names(&in_category).len(), 2);
    assert!(in_category.contains(&format!(
        r#"name="service_id" value="{}" checked"#,
        seed.a.0
    )));

    // "Dịch vụ | Nhà cung cấp" tabs carry the same search to the other list, and
    // the near-me banner keeps the category.
    let filtered = html(&e, &format!("/services?q=phun&service_id={}", seed.a.0)).await;
    assert!(filtered.contains(&format!(
        r#"href="/providers?q=phun&amp;service_id={}" data-tab="providers""#,
        seed.a.0
    )));
    assert!(filtered.contains(r#"data-tab="services" aria-current="page""#));
    assert!(in_category.contains(&format!(r#"href="/explore?service_id={}""#, seed.a.0)));
    let directory = html(&e, "/providers?q=phun").await;
    assert!(directory.contains(r#"href="/services?q=phun" data-tab="services""#));
    assert!(directory.contains(r#"data-tab="providers" aria-current="page""#));
    assert!(directory.contains(r#"href="/explore?tab=providers""#));

    let none = html(&e, "/services?q=khong-co-ai").await;
    assert!(none.contains("Không tìm thấy dịch vụ nào"));
    assert!(
        none.contains(r#"href="/providers?q=khong-co-ai""#),
        "an empty search offers the shops with the same words"
    );
    let edited = html(&e, "/services?service_id=khong&sort=rating&page=-3").await;
    assert_eq!(service_names(&edited).len(), 4);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn only_public_services_are_listed_or_shown() {
    let e = Env::new().await;
    let seed = seeded(&e).await;
    let count = |html: &str| service_names(html).len();
    assert_eq!(count(&html(&e, "/services").await), 4);

    // Hidden by an admin, deleted, or of a deactivated business: gone from
    // the listing and its page.
    service::Entity::update_many()
        .col_expr(service::Column::IsActive, Expr::value(false))
        .filter(service::Column::Id.eq(seed.repair))
        .exec(&e.t.db)
        .await
        .unwrap();
    assert_eq!(count(&html(&e, "/services").await), 3);
    let res = send(
        &e.router,
        page(&format!("/services/{}/sua-may-cay", seed.repair)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    business::Entity::update_many()
        .col_expr(business::Column::IsActive, Expr::value(false))
        .filter(business::Column::Id.eq(seed.bay_tot))
        .exec(&e.t.db)
        .await
        .unwrap();
    assert_eq!(count(&html(&e, "/services").await), 2);

    // A hidden category takes its services with it.
    category::Entity::update_many()
        .col_expr(category::Column::IsActive, Expr::value(false))
        .filter(category::Column::Id.eq(seed.b.0))
        .exec(&e.t.db)
        .await
        .unwrap();
    assert_eq!(service_names(&html(&e, "/services").await), ["Múc bùn ao"]);
    let api = json_body(
        send(
            &e.router,
            Request::get(format!("/api/explore/services?category_id={}", seed.b.0))
                .body(Body::empty())
                .unwrap(),
        )
        .await,
    )
    .await;
    assert_eq!(api["total"], 0);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn service_category_pages_have_one_url() {
    let e = Env::new().await;
    let seed = seeded(&e).await;
    let (a, slug_a) = seed.a;
    let canonical = format!("/services/category/{a}/{slug_a}");

    let listing = html(&e, &canonical).await;
    assert_eq!(service_names(&listing).len(), 2);
    assert!(listing.contains("2 dịch vụ"), "count in the hero");
    assert!(!listing.contains("list-search"));

    let res = send(&e.router, page(&format!("/services/category/{a}/cu"))).await;
    assert_eq!(res.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(res.headers()[header::LOCATION], canonical.as_str());

    let product_category = categories_of(&e, "product").await[0].0;
    for uri in [
        format!("/services/category/{product_category}/x"),
        "/services/category/0/x".to_owned(),
    ] {
        let res = send(&e.router, page(&uri)).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{uri}");
    }
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_service_has_its_own_page_with_the_contact_form_ready() {
    let e = env_with_origin().await;
    let seed = seeded(&e).await;
    let canonical = format!("/services/{}/sua-may-cay", seed.repair);

    for other in [
        format!("/services/{}", seed.repair),
        format!("/services/{}/cu", seed.repair),
    ] {
        let res = send(&e.router, page(&other)).await;
        assert_eq!(res.status(), StatusCode::MOVED_PERMANENTLY, "{other}");
        assert_eq!(res.headers()[header::LOCATION], canonical.as_str());
    }
    let res = send(&e.router, page("/services/0/x")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let html = html(&e, &canonical).await;
    assert!(html.contains("<title>Sửa máy cày | Cơ khí Bảy Tốt | Agri-Connect</title>"));
    assert!(html.contains(r#"<meta name="description" content="Sửa máy cày của Cơ khí Bảy Tốt"#));
    assert!(html.contains("300.000 ₫"));
    assert!(html.contains(r#"href="tel:0901000001""#));
    assert!(html.contains("Dịch vụ khác của Cơ khí Bảy Tốt"));
    assert!(html.contains("Sửa máy bơm"), "the provider's other service");
    assert!(html.contains(r#"name="return_to" value="service""#));
    assert!(
        html.contains(&format!(r#"<option value="{}" selected>"#, seed.repair)),
        "this service is already chosen"
    );

    // The provider page links each service to its own page.
    let res = send(&e.router, page(&format!("/providers/{}", seed.bay_tot))).await;
    let provider_url = res.headers()[header::LOCATION].to_str().unwrap().to_owned();
    assert!(body_text(send(&e.router, page(&provider_url)).await)
        .await
        .contains(&format!(r#"href="{canonical}""#)));

    // A refused form re-shows this page; a sent one lands back on it.
    let repair = seed.repair.to_string();
    let business = seed.bay_tot.to_string();
    let form = |name: &'static str| {
        post_form(
            "/contact",
            &[
                ("business_id", business.as_str()),
                ("service_id", repair.as_str()),
                ("return_to", "service"),
                ("requester_phone", "0912345678"),
                ("requester_name", name),
            ],
            None,
        )
    };
    let res = send(&e.router, form("")).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let refused = body_text(res).await;
    assert!(refused.contains("<title>Sửa máy cày | Cơ khí Bảy Tốt"));
    assert!(
        refused.contains(r#"value="0912345678""#),
        "typed phone kept"
    );

    let res = send(&e.router, form("Chú Năm")).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], canonical.as_str());
    let stored = inquiry::Entity::find().one(&e.t.db).await.unwrap().unwrap();
    assert_eq!(stored.service_id, Some(seed.repair));
    assert_eq!(stored.topic, "Sửa máy cày");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_directory_lists_every_provider_and_finds_them_by_their_services() {
    let e = Env::new().await;
    let seed = seeded(&e).await;

    let all = html(&e, "/providers").await;
    assert!(all.contains("<title>Nhà cung cấp | Agri-Connect</title>"));
    assert!(
        all.contains("<strong>4</strong> nhà cung cấp"),
        "a new sign-up without services is listed too"
    );
    // Default order: verified first, then newest.
    assert_eq!(
        provider_names(&all),
        [
            "Múc bùn Út Hiền",
            "Drone phun Ba Lúa",
            "Cơ khí Bảy Tốt",
            "Vườn mới đăng ký"
        ]
    );
    assert!(
        all.contains("phường Cai Lậy, Đồng Tháp"),
        "ward and province on the card"
    );

    assert_eq!(
        provider_names(&html(&e, "/providers?sort=name_asc").await),
        [
            "Cơ khí Bảy Tốt",
            "Drone phun Ba Lúa",
            "Múc bùn Út Hiền",
            "Vườn mới đăng ký"
        ]
    );
    assert_eq!(
        provider_names(&html(&e, "/providers?q=muc+bun").await),
        ["Múc bùn Út Hiền"]
    );
    // Found by a service's name alone.
    assert_eq!(
        provider_names(&html(&e, "/providers?q=may+cay").await),
        ["Cơ khí Bảy Tốt"]
    );
    let in_category = html(&e, &format!("/providers?service_id={}", seed.a.0)).await;
    assert_eq!(provider_names(&in_category).len(), 2);
    assert!(
        !in_category.contains("Khoảng giá"),
        "providers have no price filter"
    );

    let none = html(&e, "/providers?q=khong-co-ai").await;
    assert!(none.contains("Không tìm thấy nhà cung cấp nào"));
    assert!(none.contains(r#"href="/providers?q=khong-co-ai""#));
    assert_eq!(
        provider_names(&html(&e, "/providers?service_id=khong&sort=rating&page=-3").await).len(),
        4
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_default_order_takes_turns_so_one_shop_never_fills_the_page() {
    let e = Env::new().await;
    let seed = seeded(&e).await;
    // Bảy Tốt adds two newer services: newest first would put all four of its
    // services ahead of every other unverified provider.
    add_service(&e, seed.bay_tot, seed.a.0, "Sửa máy gặt", None, 0).await;
    add_service(&e, seed.bay_tot, seed.b.0, "Sửa máy xới", None, 0).await;

    let names = service_names(&html(&e, "/services").await);
    assert_eq!(
        &names[..3],
        ["Múc bùn ao", "Sửa máy xới", "Phun thuốc drone"],
        "every provider's first before anyone's second: {names:?}"
    );
    assert_eq!(names.len(), 6);
    // An explicit sort is kept as chosen.
    let newest = service_names(&html(&e, "/services?sort=newest").await);
    assert_eq!(&newest[..2], ["Sửa máy xới", "Sửa máy gặt"]);
    e.cleanup().await;
}
