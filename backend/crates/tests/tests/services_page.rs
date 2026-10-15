//! The provider directory and service category pages: GET filters, sorts,
//! pagination and canonical URLs. Database tests need TEST_DATABASE_URL and
//! `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{categories_of, send, Env};
use sea_orm::ConnectionTrait;

fn page(uri: &str) -> Request<Body> {
    Request::get(uri)
        .header(header::ACCEPT, "text/html")
        .body(Body::empty())
        .unwrap()
}

async fn html(e: &Env, uri: &str) -> String {
    let res = send(&e.router, page(uri)).await;
    assert_eq!(res.status(), StatusCode::OK, "{uri}");
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// Provider names in page order.
fn names(html: &str) -> Vec<String> {
    html.split(r#"<h3 class="card-title fs-6 fw-bold mb-0 flex-grow-1">"#)
        .skip(1)
        .filter_map(|s| s.split_once("</h3>").map(|(name, _)| name.to_owned()))
        .collect()
}

/// Three providers: two offering the first service category (one verified),
/// one offering the second; plus one with no active service, which the
/// directory never lists. Returns the two service categories.
async fn seeded(e: &Env) -> [(i32, String); 2] {
    let cats = categories_of(e, "service").await;
    let (a, b) = (cats[0].0, cats[1].0);
    e.t.db
        .execute_unprepared(&format!(
            "INSERT INTO businesses (name, phone, address, is_verified, created_on) VALUES
                 ('Cơ khí Bảy Tốt', '0901000001', 'Ấp 1, Cai Lậy, Tiền Giang', FALSE, now() - interval '3 day'),
                 ('Múc bùn Út Hiền', '0901000002', 'Long Xuyên, An Giang', TRUE, now() - interval '2 day'),
                 ('Drone phun Ba Lúa', '0901000003', NULL, FALSE, now() - interval '1 day'),
                 ('Chưa có dịch vụ', '0901000004', NULL, TRUE, now());
             INSERT INTO services (business_id, category_id, name)
             SELECT b.id, v.cat, v.svc FROM businesses b JOIN (VALUES
                 ('Cơ khí Bảy Tốt', {a}, 'Sửa máy cày'),
                 ('Múc bùn Út Hiền', {a}, 'Múc bùn ao'),
                 ('Drone phun Ba Lúa', {b}, 'Phun thuốc')) AS v(biz, cat, svc)
               ON v.biz = b.name;"
        ))
        .await
        .unwrap();
    [cats[0].clone(), cats[1].clone()]
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_directory_searches_filters_and_sorts_providers() {
    let e = Env::new().await;
    let [(a, _), _] = seeded(&e).await;

    let all = html(&e, "/services").await;
    assert!(all.contains("<title>Dịch vụ nông nghiệp — Agri-Connect</title>"));
    assert!(
        all.contains(r#"<meta name="description" content="Tìm nhà cung cấp dịch vụ nông nghiệp"#)
    );
    assert!(
        all.contains("<strong>3</strong> nhà cung cấp"),
        "no-service business hidden"
    );
    // Default order: verified first, then newest.
    assert_eq!(
        names(&all),
        ["Múc bùn Út Hiền", "Drone phun Ba Lúa", "Cơ khí Bảy Tốt"]
    );
    assert!(all.contains("Cai Lậy, Tiền Giang"), "district on the card");
    assert!(all.contains(r#"href="/explore""#), "near-me banner");

    let sorted = html(&e, "/services?sort=name_asc").await;
    assert_eq!(
        names(&sorted),
        ["Cơ khí Bảy Tốt", "Drone phun Ba Lúa", "Múc bùn Út Hiền"]
    );

    let searched = html(&e, "/services?q=muc+bun").await;
    assert_eq!(names(&searched), ["Múc bùn Út Hiền"]);

    let in_service = html(&e, &format!("/services?service_id={a}")).await;
    assert_eq!(names(&in_service).len(), 2);
    assert!(in_service.contains(&format!(r#"name="service_id" value="{a}" checked"#)));
    assert!(
        !in_service.contains("Khoảng giá"),
        "providers have no price filter"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn hand_edited_queries_and_empty_results() {
    let e = Env::new().await;
    seeded(&e).await;
    let page = html(&e, "/services?service_id=khong&sort=rating&page=-3").await;
    assert_eq!(names(&page).len(), 3);

    let none = html(&e, "/services?q=khong-co-ai").await;
    assert!(none.contains("Không tìm thấy nhà cung cấp nào"));
    assert!(none.contains(r#"href="/services?q=khong-co-ai""#));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn service_category_pages_have_one_url() {
    let e = Env::new().await;
    let [(a, slug_a), _] = seeded(&e).await;
    let canonical = format!("/services/category/{a}/{slug_a}");

    let listing = html(&e, &canonical).await;
    assert_eq!(names(&listing).len(), 2);
    assert!(listing.contains("2 nhà cung cấp"), "count in the hero");
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
