//! Product listings: the GET filter form, lenient query parsing, pagination
//! links and the category pages. Database tests need TEST_DATABASE_URL and
//! `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, Response, StatusCode},
};
use integration_tests::{register_provider, send, Env};
use sea_orm::{ConnectionTrait, Statement};

fn page(uri: &str) -> Request<Body> {
    Request::get(uri)
        .header(header::ACCEPT, "text/html")
        .body(Body::empty())
        .unwrap()
}

async fn html(e: &Env, uri: &str) -> String {
    let res = send(&e.router, page(uri)).await;
    assert_eq!(res.status(), StatusCode::OK, "{uri}");
    text(res).await
}

async fn text(res: Response<Body>) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// (id, slug) of the first two product categories in display order.
async fn categories(e: &Env) -> [(i32, String); 2] {
    let rows = e
        .t
        .db
        .query_all(Statement::from_string(
            e.t.db.get_database_backend(),
            "SELECT id, slug FROM categories WHERE type = 'product' ORDER BY display_order LIMIT 2",
        ))
        .await
        .unwrap();
    let pick = |i: usize| {
        let r = &rows[i];
        (
            r.try_get::<i32>("", "id").unwrap(),
            r.try_get::<String>("", "slug").unwrap(),
        )
    };
    [pick(0), pick(1)]
}

/// Three products in the first category, one in the second.
async fn seeded(e: &Env) -> [(i32, String); 2] {
    register_provider(&e.state, "nongdan", "0901234567").await;
    let cats = categories(e).await;
    let (a, b) = (cats[0].0, cats[1].0);
    e.t.db
        .execute_unprepared(&format!(
            "INSERT INTO products (business_id, category_id, name, price, unit)
             SELECT b.id, v.cat, v.name, v.price, 'kg'
             FROM businesses b, (VALUES
                 ({a}, 'Gạo ST25', 28000),
                 ({a}, 'Gạo Nàng Hoa', 22000),
                 ({a}, 'Sầu riêng Ri6', 95000),
                 ({b}, 'Phân Urê', 12000)) AS v(cat, name, price)
             WHERE b.name = 'Vườn nongdan'"
        ))
        .await
        .unwrap();
    cats
}

/// The product names in page order.
fn names(html: &str) -> Vec<String> {
    html.split("product-card__name\"><a class=\"text-decoration-none text-dark\" href=\"")
        .skip(1)
        .filter_map(|s| s.split_once("\">").map(|(_, rest)| rest))
        .filter_map(|s| s.split_once("</a>").map(|(name, _)| name.to_owned()))
        .collect()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_list_filters_by_text_category_and_price_and_sorts() {
    let e = Env::new().await;
    let [(cat_a, _), _] = seeded(&e).await;

    let all = html(&e, "/products").await;
    assert!(all.contains("<title>Sản phẩm nông nghiệp — Agri-Connect</title>"));
    assert!(all.contains("<strong>4</strong> sản phẩm"));
    assert!(all.contains(r#"<form class="list-search" method="get" action="/products""#));

    // Accent-insensitive search (S3) through the page.
    let rice = html(&e, "/products?q=gao").await;
    assert_eq!(names(&rice).len(), 2, "{:?}", names(&rice));
    assert!(
        rice.contains(r#"value="gao""#),
        "the search box keeps the text"
    );
    assert!(rice.contains("cho “gao”"));

    let in_cat = html(&e, &format!("/products?category_id={cat_a}")).await;
    assert_eq!(names(&in_cat).len(), 3);
    assert!(
        in_cat.contains(&format!(r#"value="{cat_a}" checked"#)),
        "chip stays selected"
    );
    assert!(
        in_cat.contains(r#"list-search__badge">1<"#),
        "one active filter"
    );

    let cheap = html(
        &e,
        "/products?min_price=20000&max_price=30000&sort=price_asc",
    )
    .await;
    assert_eq!(names(&cheap), ["Gạo Nàng Hoa", "Gạo ST25"]);

    let dear_first = html(&e, "/products?sort=price_desc").await;
    assert_eq!(
        names(&dear_first).first().map(String::as_str),
        Some("Sầu riêng Ri6")
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn hand_edited_queries_show_a_page_not_an_error() {
    let e = Env::new().await;
    seeded(&e).await;
    for uri in [
        "/products?min_price=abc&max_price=-1",
        "/products?category_id=999999",
        "/products?sort=cheapest&page=zero",
    ] {
        let page = html(&e, uri).await;
        assert!(
            page.contains("<strong>4</strong> sản phẩm"),
            "{uri} ignores what it cannot use"
        );
    }
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn nothing_found_offers_to_clear_the_filters_but_keeps_the_search() {
    let e = Env::new().await;
    seeded(&e).await;
    let page = html(&e, "/products?q=xoai&min_price=1").await;
    assert!(page.contains("Không tìm thấy sản phẩm nào"));
    assert!(
        page.contains(r#"href="/products?q=xoai""#),
        "reset keeps the search text"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn long_lists_page_with_links_that_keep_the_filters() {
    let e = Env::new().await;
    let [(cat_a, _), _] = seeded(&e).await;
    // 25 more in the first category: 28 there, 2 pages of 20.
    e.t.db
        .execute_unprepared(&format!(
            "INSERT INTO products (business_id, category_id, name, price)
             SELECT b.id, {cat_a}, 'Lô ' || n, 1000 * n
             FROM businesses b, generate_series(1, 25) AS n WHERE b.name = 'Vườn nongdan'"
        ))
        .await
        .unwrap();

    let first = html(&e, &format!("/products?category_id={cat_a}")).await;
    assert_eq!(names(&first).len(), 20);
    let next = format!(r#"href="/products?category_id={cat_a}&amp;page=2""#);
    assert!(first.contains(&next), "next link keeps the filter");
    assert!(first.contains(r#"aria-current="page">1<"#));

    let second = html(&e, &format!("/products?category_id={cat_a}&page=2")).await;
    assert_eq!(names(&second).len(), 8);
    assert!(
        second.contains(&format!(
            r#"href="/products?category_id={cat_a}" aria-label="Trang 1""#
        )),
        "page 1 has no page parameter"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn category_pages_have_one_url_and_only_product_categories_exist() {
    let e = Env::new().await;
    let [(cat_a, slug_a), _] = seeded(&e).await;
    let canonical = format!("/products/category/{cat_a}/{slug_a}");

    let listing = html(&e, &canonical).await;
    assert_eq!(names(&listing).len(), 3);
    assert!(
        !listing.contains("list-search"),
        "no filter panel on a category page"
    );

    let res = send(
        &e.router,
        page(&format!("/products/category/{cat_a}/ten-cu?page=2")),
    )
    .await;
    assert_eq!(res.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        res.headers()[header::LOCATION],
        format!("{canonical}?page=2").as_str()
    );

    let service_category: i32 =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "SELECT id FROM categories WHERE type = 'service' LIMIT 1",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "id")
            .unwrap();
    for uri in [
        format!("/products/category/{service_category}/x"),
        "/products/category/999999/x".to_owned(),
        "/products/category/abc/x".to_owned(),
    ] {
        let res = send(&e.router, page(&uri)).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{uri}");
    }
    e.cleanup().await;
}
