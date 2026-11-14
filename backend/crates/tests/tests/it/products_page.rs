//! Product listings: the GET filter form, lenient query parsing, pagination
//! links and the category pages. Database tests need TEST_DATABASE_URL and
//! `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, Response, StatusCode},
};
use integration_tests::{categories_of, register_provider, send, Env};
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

/// Three products in the first category, one in the second.
async fn seeded(e: &Env) -> [(i32, String); 2] {
    register_provider(&e.state, "nongdan", "0901234567").await;
    let cats = categories_of(e, "product").await;
    let (a, b) = (cats[0].0, cats[1].0);
    e.t.db
        .execute_unprepared(&format!(
            "INSERT INTO products (business_id, supply_type_id, name, price, unit)
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
    [cats[0].clone(), cats[1].clone()]
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

    let all = html(&e, "/supplies").await;
    assert!(all.contains("<title>Vật tư nông nghiệp — Agri-Connect</title>"));
    assert!(all.contains("<strong>4</strong> vật tư"));
    assert!(all.contains(r#"<form class="list-search" method="get" action="/supplies""#));

    // Accent-insensitive search through the page.
    let rice = html(&e, "/supplies?q=gao").await;
    assert_eq!(names(&rice).len(), 2, "{:?}", names(&rice));
    assert!(
        rice.contains(r#"value="gao""#),
        "the search box keeps the text"
    );
    assert!(rice.contains("cho “gao”"));

    let in_cat = html(&e, &format!("/supplies?category_id={cat_a}")).await;
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
        "/supplies?min_price=20000&max_price=30000&sort=price_asc",
    )
    .await;
    assert_eq!(names(&cheap), ["Gạo Nàng Hoa", "Gạo ST25"]);

    let dear_first = html(&e, "/supplies?sort=price_desc").await;
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
        "/supplies?min_price=abc&max_price=-1",
        "/supplies?category_id=999999",
        "/supplies?sort=cheapest&page=zero",
    ] {
        let page = html(&e, uri).await;
        assert!(
            page.contains("<strong>4</strong> vật tư"),
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
    let page = html(&e, "/supplies?q=xoai&min_price=1").await;
    assert!(page.contains("Không tìm thấy vật tư nào"));
    assert!(
        page.contains(r#"href="/supplies?q=xoai""#),
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
            "INSERT INTO products (business_id, supply_type_id, name, price)
             SELECT b.id, {cat_a}, 'Lô ' || n, 1000 * n
             FROM businesses b, generate_series(1, 25) AS n WHERE b.name = 'Vườn nongdan'"
        ))
        .await
        .unwrap();

    let first = html(&e, &format!("/supplies?category_id={cat_a}")).await;
    assert_eq!(names(&first).len(), 20);
    let next = format!(r#"href="/supplies?category_id={cat_a}&amp;page=2""#);
    assert!(first.contains(&next), "next link keeps the filter");
    assert!(first.contains(r#"aria-current="page">1<"#));

    let second = html(&e, &format!("/supplies?category_id={cat_a}&page=2")).await;
    assert_eq!(names(&second).len(), 8);
    assert!(
        second.contains(&format!(
            r#"href="/supplies?category_id={cat_a}" aria-label="Trang 1""#
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
    let canonical = format!("/supplies/category/{cat_a}/{slug_a}");

    let listing = html(&e, &canonical).await;
    assert_eq!(names(&listing).len(), 3);
    assert!(
        !listing.contains("list-search"),
        "no filter panel on a category page"
    );

    let res = send(
        &e.router,
        page(&format!("/supplies/category/{cat_a}/ten-cu?page=2")),
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
                "SELECT id FROM service_types LIMIT 1",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "id")
            .unwrap();
    for uri in [
        format!("/supplies/category/{service_category}/x"),
        "/supplies/category/999999/x".to_owned(),
        "/supplies/category/abc/x".to_owned(),
    ] {
        let res = send(&e.router, page(&uri)).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{uri}");
    }
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn unpriced_products_come_last_in_both_price_orders() {
    let e = Env::new().await;
    seeded(&e).await;
    e.t.db
        .execute_unprepared(
            "INSERT INTO products (business_id, supply_type_id, name)
             SELECT business_id, supply_type_id, 'Liên hệ giá' FROM products LIMIT 1",
        )
        .await
        .unwrap();
    for sort in ["price_desc", "price_asc"] {
        let shown = names(&html(&e, &format!("/supplies?sort={sort}")).await);
        assert_eq!(
            shown.last().map(String::as_str),
            Some("Liên hệ giá"),
            "{sort}: {shown:?}"
        );
    }
    assert_eq!(
        names(&html(&e, "/supplies?sort=price_desc").await)
            .first()
            .map(String::as_str),
        Some("Sầu riêng Ri6")
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn rows_with_equal_sort_values_never_repeat_across_pages() {
    let e = Env::new().await;
    seeded(&e).await;
    // Like demo data: many rows, one created_on (a single transaction).
    e.t.db
        .execute_unprepared(
            "INSERT INTO products (business_id, supply_type_id, name, price, created_on)
             SELECT p.business_id, p.supply_type_id, 'Lô ' || g, 1000, '2026-01-01'
             FROM (SELECT business_id, supply_type_id FROM products LIMIT 1) p,
                  generate_series(1, 45) g;
             UPDATE products SET created_on = '2026-01-01'",
        )
        .await
        .unwrap();
    let mut seen = Vec::new();
    for page in 1..=3 {
        seen.extend(names(&html(&e, &format!("/supplies?page={page}")).await));
    }
    let total = seen.len();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), total, "a row appeared twice");
    assert_eq!(total, 49);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_page_past_the_end_redirects_to_the_last_page() {
    let e = Env::new().await;
    let [(cat, slug), _] = seeded(&e).await;
    let res = send(&e.router, page("/supplies?q=g%E1%BA%A1o&page=9")).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/supplies?q=g%E1%BA%A1o");
    let res = send(
        &e.router,
        page(&format!("/supplies/category/{cat}/{slug}?page=4")),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        res.headers()[header::LOCATION],
        format!("/supplies/category/{cat}/{slug}").as_str()
    );
    e.cleanup().await;
}
