//! Supplies ("vật tư") apart from farm produce ("nông sản"): what a farmer
//! buys lives at /supplies (the former /products addresses move for good),
//! the price board is grouped by commodity (cây trồng, vật nuôi, its own admin list),
//! a crop must be of one, and a supply may say which one it is for. The home page names its two rows of shortcuts.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, categories_of, env_with_origin, first_admin, get_page, install_admin, post_form,
    send, Env,
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

async fn moved_to(e: &Env, from: &str, cookie: Option<&str>) -> String {
    let res = send(&e.router, get_page(from, cookie)).await;
    assert_eq!(res.status(), StatusCode::MOVED_PERMANENTLY, "{from}");
    res.headers()[header::LOCATION].to_str().unwrap().to_owned()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_former_product_addresses_move_to_the_supplies() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    assert_eq!(
        moved_to(&e, "/products?q=ph%C3%A2n&page=2", None).await,
        "/supplies?q=ph%C3%A2n&page=2",
        "the query is kept"
    );
    let (id, slug) = categories_of(&e, "product").await[0].clone();
    assert_eq!(
        moved_to(&e, &format!("/products/category/{id}/{slug}"), None).await,
        format!("/supplies/category/{id}/{slug}")
    );
    assert_eq!(
        moved_to(&e, "/admin/products?provider_id=3", Some(&admin)).await,
        "/admin/supplies?provider_id=3"
    );
    assert_eq!(
        moved_to(&e, "/admin/products/7", Some(&admin)).await,
        "/admin/supplies/7"
    );
    assert_eq!(
        moved_to(&e, "/provider/products/new", None).await,
        "/provider/supplies/new"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn commodities_are_their_own_list_and_every_crop_has_one() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    // What farmers raise has its own screen; the type tabs know nothing of it.
    let res = send(&e.router, get_page("/admin/commodities", Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_text(res).await;
    assert!(html.contains("Sầu riêng") && html.contains("Cá tra"));
    let types = body_text(
        send(
            &e.router,
            get_page("/admin/categories?type=product", Some(&admin)),
        )
        .await,
    )
    .await;
    assert!(types.contains("Giống") && !types.contains("Sầu riêng"));

    // A new commodity, then a crop of it; a crop with none is refused.
    let res = send(
        &e.router,
        post_form(
            "/admin/commodities",
            &[("name", "Cà phê"), ("sector", "crop")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let commodity = scalar(
        &e,
        "SELECT id::text AS v FROM commodities WHERE name = 'Cà phê'",
    )
    .await;
    let res = send(
        &e.router,
        post_form(
            "/admin/crops",
            &[("name", "Cà phê nhân"), ("unit", "kg")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res)
        .await
        .contains("Vui lòng chọn cây trồng, vật nuôi."));
    let res = send(
        &e.router,
        post_form(
            "/admin/crops",
            &[
                ("name", "Cà phê nhân"),
                ("commodity_id", &commodity),
                ("unit", "kg"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let list =
        body_text(send(&e.router, get_page("/admin/prices?tab=crops", Some(&admin))).await).await;
    assert!(list.contains("Cà phê nhân") && list.contains("Cà phê"));

    // A commodity needs a sector.
    let res = send(
        &e.router,
        post_form(
            "/admin/commodities",
            &[("name", "Hồ tiêu"), ("sector", "")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // An id that is no commodity.
    let res = send(
        &e.router,
        post_form(
            "/admin/crops",
            &[
                ("name", "Hồ tiêu"),
                ("commodity_id", "999999"),
                ("unit", "kg"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_supply_says_what_it_is_for_and_the_board_leads_to_it() {
    let e = env_with_origin().await;
    let cookie = install_admin(&e).await;
    // Loaded the way an admin does it, from the demo data page.
    let res = send(
        &e.router,
        post_form("/setup/seed", &[("action", "load")], Some(&cookie)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let rice = scalar(
        &e,
        "SELECT id::text AS v FROM commodities WHERE name = 'Lúa'",
    )
    .await;

    // For rice: the rice seed and what every crop uses, never pig feed.
    let page = body_text(
        send(
            &e.router,
            get_page(&format!("/supplies?commodity={rice}"), None),
        )
        .await,
    )
    .await;
    assert!(page.contains("Lúa giống ST25 cấp xác nhận"));
    assert!(
        page.contains("Phân NPK 16-16-8 Bình Điền"),
        "for every crop"
    );
    assert!(!page.contains("Cám heo thịt 25 kg"));
    assert!(!page.contains("Cây giống sầu riêng Ri6"));
    assert!(
        page.contains(r#"name="commodity" value=""#),
        "the filter form keeps it"
    );

    // Fish feed is aquatic though its type serves every sector.
    let aquatic =
        body_text(send(&e.router, get_page("/supplies", Some("sector=aquatic"))).await).await;
    assert!(aquatic.contains("Thức ăn cá tra 40 đạm"));
    assert!(!aquatic.contains("Cám heo thịt 25 kg"));

    // The price board's rice section leads to the supplies for rice.
    let board = body_text(send(&e.router, get_page("/prices", Some("sector=crop"))).await).await;
    assert!(board.contains(&format!("/supplies?commodity={rice}")));
    assert!(board.contains("Vật tư cho Lúa"));

    // The admin's list says what it is for.
    let list = body_text(send(&e.router, get_page("/admin/supplies", Some(&cookie))).await).await;
    assert!(list.contains("<th>Dùng cho</th>") && list.contains(">Lúa</a>"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_home_page_names_its_rows_and_the_board_follows_the_sector() {
    let e = env_with_origin().await;
    let admin = first_admin(&e.state).await;
    e.state.usecases.setup.load_demo(&admin).await.unwrap();
    let home = body_text(send(&e.router, get_page("/", None)).await).await;
    let hire = home.find("Thuê làm").expect("the services row");
    let buy = home.find("Mua vật tư").expect("the supplies row");
    assert!(hire < buy, "hiring first, the farmer's main errand");

    // The JSON board names its commodities; a sector keeps its own.
    let res = send(
        &e.router,
        get_page("/api/explore/prices?sector=livestock", None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let board: serde_json::Value = serde_json::from_str(&body_text(res).await).unwrap();
    let commodities: Vec<&str> = board["commodities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["name"].as_str().unwrap())
        .collect();
    assert_eq!(commodities, ["Heo", "Bò", "Gà"]);
    assert!(board["crops"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| board["commodities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == c["commodity_id"])));
    let page = body_text(send(&e.router, get_page("/prices", Some("sector=aquatic"))).await).await;
    assert!(page.contains("Cá tra") && !page.contains("Heo hơi"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_json_api_lists_and_keeps_the_commodities() {
    use axum::{body::Body, http::Request};
    let e = env_with_origin().await;
    install_admin(&e).await;
    let token = e
        .state
        .usecases
        .auth
        .login(
            "quantri".into(),
            "matkhau123".into(),
            integration_tests::CLIENT,
        )
        .await
        .unwrap()
        .token;
    let json = |method: &str, uri: &str, body: &str| {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap()
    };

    let res = send(
        &e.router,
        json(
            "POST",
            "/api/admin/commodities",
            r#"{"name":"Cà phê","sector":"crop","display_order":3}"#,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created: serde_json::Value = serde_json::from_str(&body_text(res).await).unwrap();
    assert_eq!(created["sector"], "crop");
    let id = created["id"].as_i64().unwrap();

    // A taken name is a field error, never a 500.
    let res = send(
        &e.router,
        json(
            "POST",
            "/api/admin/commodities",
            r#"{"name":"Cà phê","sector":"crop"}"#,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res)
        .await
        .contains("Đã có cây trồng, vật nuôi trùng tên."));

    // Hidden, it leaves the public list.
    let res = send(
        &e.router,
        json(
            "PATCH",
            &format!("/api/admin/commodities/{id}/status"),
            r#"{"is_active":false}"#,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let public = body_text(send(&e.router, get_page("/api/explore/commodities", None)).await).await;
    assert!(public.contains("Sầu riêng") && !public.contains("Cà phê"));

    // Only the admin keeps them.
    let res = send(
        &e.router,
        Request::builder()
            .method("GET")
            .uri("/api/admin/commodities")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_business_lists_a_capped_number_of_supplies() {
    use application::constants::PRODUCTS_PER_BUSINESS_DEFAULT;
    use axum::{body::Body, http::Request};
    use integration_tests::{create_product, json_body, register_provider, with_product_category};

    let e = Env::new().await;
    let token = register_provider(&e.state, "nongdan", "0901234567").await;
    let body = with_product_category(&e, r#"{"name":"NPK 16-16-8","price":15000}"#).await;
    let first = create_product(&e, &token, &body).await;
    // One short of the cap, a hidden one among them: hidden ones count.
    e.t.db
        .execute_unprepared(&format!(
            "INSERT INTO products (business_id, supply_type_id, name, is_active)
             SELECT p.business_id, p.supply_type_id, 'NPK ' || n, n > 1
             FROM products p, generate_series(1, {}) AS n WHERE p.id = {first}",
            PRODUCTS_PER_BUSINESS_DEFAULT - 2
        ))
        .await
        .unwrap();
    create_product(&e, &token, &body).await;

    let one_more = || {
        Request::post("/api/provider/products")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.clone()))
            .unwrap()
    };
    let res = send(&e.router, one_more()).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(json_body(res).await["code"], "product_limit_reached");
    assert_eq!(
        scalar(&e, "SELECT count(*)::text AS v FROM products").await,
        PRODUCTS_PER_BUSINESS_DEFAULT.to_string()
    );

    // A deleted one frees its place.
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE products SET deleted_at = now() WHERE id = {first}"
        ))
        .await
        .unwrap();
    assert_eq!(
        send(&e.router, one_more()).await.status(),
        StatusCode::CREATED
    );
    e.cleanup().await;
}
