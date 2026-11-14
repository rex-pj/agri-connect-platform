//! Farming beyond crops: the sectors are data the admin keeps (none shown,
//! no switch); the visitor's sector (crops, livestock, aquaculture in the
//! demo data) narrows the categories and lists but never the home page,
//! searches find a thing under either of its names, the price board has a
//! section per commodity, and the admin's alerts reach the home page.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use chrono::{Duration, Utc};
use integration_tests::{
    body_text, env_with_origin, first_admin, get_page, install_admin, json_body, post_form, send,
    sign_in, Env,
};

const LIVESTOCK: &str = "sector=livestock";
const AQUATIC: &str = "sector=aquatic";

async fn page(e: &Env, path: &str, cookie: Option<&str>) -> String {
    let res = send(&e.router, get_page(path, cookie)).await;
    let status = res.status();
    if status != StatusCode::OK {
        panic!("{path}: {status}: {}", body_text(res).await);
    }
    body_text(res).await
}

/// An admin and the whole demo data set (every sector).
async fn with_demo(e: &Env) -> String {
    let actor = first_admin(&e.state).await;
    e.state.usecases.setup.load_demo(&actor).await.unwrap();
    sign_in(e, "quantri", "matkhau123").await
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_sector_choice_is_kept_and_narrows_the_lists() {
    let e = env_with_origin().await;
    with_demo(&e).await;

    let res = send(
        &e.router,
        post_form(
            "/sector",
            &[("sector", "livestock"), ("back", "/services")],
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/services");
    let cookie = res.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.starts_with("sector=livestock;"), "{cookie}");
    let res = send(
        &e.router,
        post_form("/sector", &[("sector", "forestry"), ("back", "/")], None),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "only the sectors we serve"
    );

    // Every sector: crop spraying is there.
    let all = page(&e, "/services", None).await;
    assert!(all.contains("Phun thuốc bằng drone"));
    // Livestock: the vet and what serves every sector, not crop spraying.
    let livestock = page(&e, "/services", Some(LIVESTOCK)).await;
    assert!(livestock.contains("Tiêm phòng heo, gà vịt tại nhà"));
    assert!(livestock.contains("Múc bùn kênh mương"), "every sector's");
    assert!(!livestock.contains("Phun thuốc bằng drone"));
    assert!(livestock.contains(r#"value="livestock" aria-pressed="true""#));

    // The home page has no switch and never follows the choice: its tiles
    // and featured services are every sector's.
    let home = page(&e, "/", Some(AQUATIC)).await;
    assert!(!home.contains("sector-switch"));
    let strip = home
        .split("cat-strip")
        .nth(1)
        .and_then(|rest| rest.split("</ul>").next())
        .expect("the category strip");
    assert!(strip.contains("Nạo vét"), "every sector's");
    assert!(strip.contains("Phun thuốc"), "a crop category all the same");
    assert!(home.contains("Phun thuốc bằng drone"));
    // Nor does the near-me page, which has no switch either.
    let near = page(&e, "/explore", Some(AQUATIC)).await;
    assert!(!near.contains("sector-switch"));
    assert!(near.contains("Phun thuốc"), "every sector's tiles");

    // The directory: providers serving the sector, and those with nothing
    // yet (a new sign-up is still found); a search is never narrowed.
    let directory = page(&e, "/providers", Some(LIVESTOCK)).await;
    assert!(
        directory.contains(r#"value="livestock" aria-pressed="true""#),
        "the switch says what narrows the directory"
    );
    assert!(directory.contains("Thú y Bảy Lộc"));
    assert!(directory.contains("Cơ sở Năm Hùng"), "nothing listed yet");
    assert!(!directory.contains("Dịch vụ Trần Thị Bình"), "crops only");
    let searched = page(&e, "/providers?q=Tr%E1%BA%A7n", Some(LIVESTOCK)).await;
    assert!(
        searched.contains("Dịch vụ Trần Thị Bình"),
        "a search finds all"
    );

    // A chosen category is the filter: a shared link still shows its rows.
    let products = page(&e, "/supplies", Some(AQUATIC)).await;
    assert!(products.contains("Tôm thẻ giống PL12"));
    assert!(!products.contains("Cám heo thịt 25 kg"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_json_api_takes_the_sector_and_lists_the_alerts() {
    let e = env_with_origin().await;
    with_demo(&e).await;

    let json = |path: &'static str| {
        let router = e.router.clone();
        async move {
            let res = send(&router, get_page(path, None)).await;
            (res.status(), json_body(res).await)
        }
    };
    let (status, body) = json("/api/explore/alerts?sector=forestry").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid_sector");

    // The demo alerts: each sector sees its own and none of another's.
    let (status, body) = json("/api/explore/alerts?sector=aquatic").await;
    assert_eq!(status, StatusCode::OK);
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["Nắng nóng, ao tôm dễ thiếu oxy"]);
    let (_, all) = json("/api/explore/alerts").await;
    assert_eq!(all.as_array().unwrap().len(), 3);

    let (_, services) = json("/api/explore/services?sector=livestock").await;
    let names: Vec<&str> = services["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"Tiêm phòng heo, gà vịt tại nhà"),
        "{names:?}"
    );
    assert!(!names.contains(&"Phun thuốc bằng drone"), "{names:?}");
    let (_, featured) = json("/api/explore/featured?sector=aquatic").await;
    assert!(featured["featured_services"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["name"] != "Phun thuốc bằng drone"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_search_finds_a_thing_under_either_name_and_prices_cover_every_sector() {
    let e = env_with_origin().await;
    with_demo(&e).await;

    // "lợn" is the North's "heo".
    let found = page(&e, "/search?q=l%E1%BB%A3n", None).await;
    assert!(found.contains("Men tiêu hoá cho heo"), "lợn finds heo");

    // A search covers every sector whatever the cookie says; the links over
    // the results narrow this one search, and keep the words and the tab.
    let found = page(&e, "/search?q=heo&tab=products", Some(AQUATIC)).await;
    assert!(
        found.contains("Men tiêu hoá cho heo"),
        "never narrowed unseen"
    );
    assert!(found.contains(r#"href="/search?q=heo&amp;tab=products&amp;sector=crop""#));
    assert!(!found.contains("sector=crop\" aria-current"));
    let crop = page(&e, "/search?q=heo&sector=crop", None).await;
    assert!(!crop.contains("Men tiêu hoá cho heo"), "livestock's");
    assert!(crop.contains(r#"sector=crop" aria-current="true""#));
    assert!(crop.contains(r#"href="/search?q=heo&amp;tab=services&amp;sector=crop""#));
    let livestock = page(&e, "/search?q=heo&sector=livestock", None).await;
    assert!(livestock.contains("Men tiêu hoá cho heo"));
    // The lists take the same links once words are typed (the cookie's switch
    // narrows nothing then), so "see more" keeps the sector.
    let supplies = page(&e, "/supplies?q=heo", Some(AQUATIC)).await;
    assert!(supplies.contains("Men tiêu hoá cho heo"));
    assert!(supplies.contains(r#"href="/supplies?q=heo&amp;sector=crop""#));
    assert!(
        !supplies.contains(r#"action="/sector""#),
        "links, not the switch"
    );
    let supplies = page(&e, "/supplies?q=heo&sector=crop", None).await;
    assert!(!supplies.contains("Men tiêu hoá cho heo"));
    assert!(
        supplies.contains(r#"name="sector" value="crop""#),
        "the form keeps it"
    );
    let shops = page(&e, "/providers?q=Tr%E1%BA%A7n&sector=livestock", None).await;
    assert!(!shops.contains("Dịch vụ Trần Thị Bình"), "crops only");
    let services = page(&e, "/services?q=heo&sector=crop", None).await;
    assert!(!services.contains("Tiêm phòng heo, gà vịt tại nhà"));
    assert!(services.contains(r#"sector=crop" aria-current="true""#));
    // Without words `sector=` is nothing: the visitor's switch rules.
    let plain = page(&e, "/services?sector=crop", None).await;
    assert!(plain.contains(r#"action="/sector""#));
    assert!(plain.contains("Tiêm phòng heo, gà vịt tại nhà"));
    let unknown = page(&e, "/search?q=heo&sector=forestry", None).await;
    assert!(
        unknown.contains("Men tiêu hoá cho heo"),
        "unknown: every sector"
    );

    let prices = page(&e, "/prices", None).await;
    for section in ["Lúa", "Heo", "Gà", "Tôm", "Cá tra"] {
        assert!(prices.contains(section), "{section}");
    }
    assert!(prices.contains("Heo hơi") && prices.contains("Cá tra"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_alert_reaches_the_home_page_while_on() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let today = (Utc::now() + Duration::hours(7)).date_naive();
    let day = |d: i64| (today + Duration::days(d)).format("%Y-%m-%d").to_string();

    // Every rule is checked at once and the sheet comes back open.
    let res = send(
        &e.router,
        post_form(
            "/admin/alerts",
            &[
                ("kind", "flood"),
                ("title", ""),
                ("starts_on", &day(2)),
                ("ends_on", &day(1)),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Vui lòng chọn loại cảnh báo."));
    assert!(html.contains("Vui lòng nhập tiêu đề cảnh báo."));
    assert!(html.contains("Ngày kết thúc phải sau hoặc bằng ngày bắt đầu."));

    let res = send(
        &e.router,
        post_form(
            "/admin/alerts",
            &[
                ("kind", "disease"),
                ("sector", "livestock"),
                ("province_code", ""),
                ("title", "Dịch tả heo châu Phi"),
                ("body", "Không mua heo không rõ nguồn gốc."),
                ("starts_on", &day(0)),
                ("ends_on", &day(7)),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let alert_page = res.headers()[header::LOCATION].to_str().unwrap().to_owned();

    assert!(page(&e, "/", None).await.contains("Dịch tả heo châu Phi"));
    assert!(
        page(&e, "/", Some(AQUATIC))
            .await
            .contains("Dịch tả heo châu Phi"),
        "the home page shows every sector's alerts, whatever was chosen on a list"
    );

    // Turned off, it leaves the home page at once.
    let res = send(
        &e.router,
        post_form(
            &format!("{alert_page}/status"),
            &[("is_active", "false"), ("reason", "hết dịch")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(!page(&e, "/", None).await.contains("Dịch tả heo châu Phi"));
    let list = page(&e, "/admin/alerts", Some(&admin)).await;
    assert!(
        list.contains("Dịch tả heo châu Phi"),
        "kept in the admin list"
    );
    e.cleanup().await;
}

/// One text value (`AS v`) read straight from the database.
async fn scalar(e: &Env, sql: &str) -> String {
    use sea_orm::{ConnectionTrait, Statement};
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

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_sectors_are_data_the_admin_keeps() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    // A new sector: its code is made from its name, the switch offers it,
    // the JSON API lists it, a commodity may be in it.
    let res = send(
        &e.router,
        post_form(
            "/admin/sectors",
            &[
                ("name", "Lâm nghiệp"),
                ("icon", "fa-tree"),
                ("display_order", "4"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let id = scalar(
        &e,
        "SELECT id::text AS v FROM sectors WHERE code = 'lam-nghiep'",
    )
    .await;
    // Its list and its own page.
    let list = page(&e, "/admin/sectors", Some(&admin)).await;
    assert!(list.contains("Lâm nghiệp") && list.contains("lam-nghiep"));
    assert!(page(&e, &format!("/admin/sectors/{id}"), Some(&admin))
        .await
        .contains("lam-nghiep"));
    let services = page(&e, "/services", None).await;
    assert!(
        services.contains(r#"value="lam-nghiep""#),
        "the switch offers it"
    );
    assert!(services.contains("fa-tree"));
    let res = send(&e.router, get_page("/api/explore/sectors", None)).await;
    let codes: Vec<String> = json_body(res)
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["code"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(codes, ["crop", "livestock", "aquatic", "lam-nghiep"]);
    let res = send(
        &e.router,
        post_form(
            "/admin/commodities",
            &[("name", "Keo lai"), ("sector", "lam-nghiep")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let list = page(&e, "/admin/commodities", Some(&admin)).await;
    assert!(list.contains("Keo lai") && list.contains("Lâm nghiệp"));

    // Renamed: the code stays (cookies and links keep it). A taken name is
    // refused.
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/sectors/{id}"),
            &[
                ("name", "Lâm sản"),
                ("icon", "fa-tree"),
                ("display_order", "4"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        scalar(
            &e,
            &format!("SELECT code AS v FROM sectors WHERE id = {id}")
        )
        .await,
        "lam-nghiep"
    );
    let res = send(
        &e.router,
        post_form("/admin/sectors", &[("name", "Chăn nuôi")], Some(&admin)),
    )
    .await;
    assert_ne!(res.status(), StatusCode::SEE_OTHER);
    assert!(body_text(res).await.contains("Đã có ngành trùng tên."));

    // Hidden: gone from the switch and the filters; its commodity stays in it.
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/sectors/{id}/status"),
            &[("is_active", "false")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(!page(&e, "/services", None)
        .await
        .contains(r#"value="lam-nghiep""#));
    let res = send(
        &e.router,
        post_form("/sector", &[("sector", "lam-nghiep"), ("back", "/")], None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(page(&e, "/admin/commodities", Some(&admin))
        .await
        .contains("Lâm sản"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn with_no_sector_shown_there_is_no_switch() {
    use sea_orm::{ConnectionTrait, Statement};
    let e = env_with_origin().await;
    assert!(page(&e, "/services", None).await.contains("sector-switch"));
    e.t.db
        .execute(Statement::from_string(
            e.t.db.get_database_backend(),
            "UPDATE sectors SET is_active = FALSE".to_owned(),
        ))
        .await
        .unwrap();
    let list = page(&e, "/services", Some(LIVESTOCK)).await;
    assert!(!list.contains("sector-switch"), "nothing to choose from");
    e.cleanup().await;
}
