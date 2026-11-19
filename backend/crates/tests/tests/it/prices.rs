//! Crop prices: the admin enters the day's price of a crop in an area (again
//! the same day replaces it), the public page shows each crop's latest fresh
//! price per area with its move, a stale or hidden one is not shown, and
//! only `price.manage` may enter one. Database tests need TEST_DATABASE_URL
//! and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{
    body_text, env_with_origin, get_page, install_admin, json_body, post_form, register_provider,
    send, Env, TEST_ORIGIN,
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

async fn exec(e: &Env, sql: &str) {
    e.t.db.execute_unprepared(sql).await.unwrap();
}

/// A day relative to today in Vietnam, as `YYYY-MM-DD`.
async fn vn_day(e: &Env, offset_days: i32) -> String {
    scalar(
        e,
        &format!(
            "SELECT ((NOW() AT TIME ZONE 'UTC') + INTERVAL '7 hours' \
             + INTERVAL '{offset_days} days')::date::text AS v"
        ),
    )
    .await
}

async fn id_of(e: &Env, table: &str, column: &str, value: &str) -> String {
    scalar(
        e,
        &format!("SELECT id::text AS v FROM {table} WHERE {column} = '{value}'"),
    )
    .await
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_admin_enters_prices_and_farmers_see_the_fresh_ones() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let rice = id_of(&e, "crops", "name", "Lúa OM 18").await;
    let jackfruit = id_of(&e, "crops", "name", "Mít Thái").await;
    let cai_lay = id_of(&e, "locations", "label", "Đồng Tháp — Cai Lậy").await;
    let today = vn_day(&e, 0).await;
    let tomorrow = vn_day(&e, 1).await;

    // A high price under the low one, a day after today: refused, every box
    // at once, and the sheet comes back open with what was typed.
    let res = send(
        &e.router,
        post_form(
            "/admin/prices",
            &[
                ("crop_id", &rice),
                ("location_id", &cai_lay),
                ("observed_on", &tomorrow),
                ("price_low", "6.400"),
                ("price_high", "6.200"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("không sau hôm nay"), "the date message");
    assert!(
        html.contains("lớn hơn hoặc bằng giá thấp"),
        "the range message"
    );
    assert!(html.contains("6.400"), "the typed value comes back");

    // Yesterday's price straight in the table, then today's through the form,
    // twice: the second replaces the first.
    exec(
        &e,
        &format!(
            "INSERT INTO crop_prices (crop_id, location_id, observed_on, price_low, price_high) \
             VALUES ({rice}, {cai_lay}, '{}', 6000, 6200)",
            vn_day(&e, -1).await
        ),
    )
    .await;
    for (low, high) in [("6.100", "6.300"), ("6.200", "6.400")] {
        let res = send(
            &e.router,
            post_form(
                "/admin/prices",
                &[
                    ("crop_id", &rice),
                    ("location_id", &cai_lay),
                    ("observed_on", &today),
                    ("price_low", low),
                    ("price_high", high),
                    ("note", "Thuong lai tai ruong"),
                ],
                Some(&admin),
            ),
        )
        .await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
    }
    assert_eq!(
        scalar(
            &e,
            &format!("SELECT COUNT(*)::text AS v FROM crop_prices WHERE observed_on = '{today}'")
        )
        .await,
        "1",
        "the same crop, area and day is one price"
    );
    // A jackfruit price older than PRICE_FRESH_DAYS.
    exec(
        &e,
        &format!(
            "INSERT INTO crop_prices (crop_id, location_id, observed_on, price_low, price_high) \
             VALUES ({jackfruit}, {cai_lay}, '{}', 30000, 32000)",
            vn_day(&e, -30).await
        ),
    )
    .await;

    // The public page: today's rice price, up 200 on yesterday's middle; no
    // stale jackfruit.
    let html = body_text(send(&e.router, get_page("/prices", None)).await).await;
    assert!(html.contains("Lúa OM 18"));
    assert!(html.contains("6.200 – 6.400 ₫"), "the range");
    assert!(html.contains("Tăng 200 đồng"), "the move since yesterday");
    assert!(!html.contains("Mít Thái"), "a stale price is not shown");
    // The home page leads there.
    let html = body_text(send(&e.router, get_page("/", None)).await).await;
    assert!(html.contains("Giá nông sản hôm nay") && html.contains("Lúa OM 18"));
    // The JSON board, one area.
    let board = json_body(
        send(
            &e.router,
            Request::get(format!("/api/explore/prices?location_id={cai_lay}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await,
    )
    .await;
    assert_eq!(board["crops"][0]["quotes"][0]["change"], 200);
    assert_eq!(board["crops"][0]["quotes"][0]["price_high"], 6400);

    // Only `price.manage` enters prices: a provider gets 403.
    let token = register_provider(&e.state, "vuonlua", "0907000111").await;
    let res = send(
        &e.router,
        Request::post("/api/admin/prices")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, TEST_ORIGIN)
            .body(Body::from(format!(
                r#"{{"crop_id":{rice},"location_id":{cai_lay},"observed_on":"{today}","price_low":1}}"#
            )))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // A crop's name is unique; hiding a crop takes its prices off the page.
    let commodity = scalar(
        &e,
        &format!("SELECT commodity_id::text AS v FROM crops WHERE id = {rice}"),
    )
    .await;
    let res = send(
        &e.router,
        post_form(
            "/admin/crops",
            &[
                ("name", "Lúa OM 18"),
                ("commodity_id", &commodity),
                ("unit", "kg"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains("Đã có nông sản với tên này"));
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/crops/{rice}/status"),
            &[("is_active", "false"), ("reason", "Het mua")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let html = body_text(send(&e.router, get_page("/prices", None)).await).await;
    assert!(!html.contains("Lúa OM 18"), "a hidden crop is not shown");

    // Deleting a price lands back on the list, and the price is gone.
    let price = scalar(
        &e,
        &format!("SELECT id::text AS v FROM crop_prices WHERE observed_on = '{today}'"),
    )
    .await;
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/prices/{price}/delete"),
            &[("reason", "Nhap nham")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/admin/prices");
    assert_eq!(
        scalar(
            &e,
            &format!("SELECT COUNT(*)::text AS v FROM crop_prices WHERE id = {price}")
        )
        .await,
        "0"
    );
    // The admin pages render (with the delete confirmation open).
    let res = send(
        &e.router,
        get_page("/admin/prices?delete=999999", Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = send(
        &e.router,
        get_page(&format!("/admin/crops/{rice}"), Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
}
