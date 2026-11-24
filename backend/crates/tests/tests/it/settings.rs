//! The platform settings (`/admin/settings`, `/api/admin/settings`): a
//! group is saved, refused, audited and put back on its defaults, and each
//! setting reaches what it governs on the next request: the footer's help
//! lines, sign-ups, job posts, the listing caps. Also each commodity's own
//! price freshness.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use image::ImageEncoder;
use integration_tests::{
    body_text, categories_of, env_with_origin, get_page, install_admin, json_body, post_form,
    post_multipart_files, register_provider, send, Env, TEST_ORIGIN,
};
use sea_orm::{ConnectionTrait, Statement};

async fn page(e: &Env, path: &str, cookie: Option<&str>) -> String {
    let res = send(&e.router, get_page(path, cookie)).await;
    let status = res.status();
    if status != StatusCode::OK {
        panic!("{path}: {status}: {}", body_text(res).await);
    }
    body_text(res).await
}

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

/// POST of one card of the settings page.
async fn save(
    e: &Env,
    admin: &str,
    pairs: &[(&str, &str)],
) -> axum::http::Response<axum::body::Body> {
    send(&e.router, post_form("/admin/settings", pairs, Some(admin))).await
}

async fn bearer(e: &Env, username: &str, password: &str) -> String {
    let req = Request::post("/api/auth/tokens")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, TEST_ORIGIN)
        .body(Body::from(format!(
            r#"{{"username":"{username}","password":"{password}"}}"#
        )))
        .unwrap();
    json_body(send(&e.router, req).await).await["token"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_group_is_saved_refused_audited_and_put_back() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    // The first tab by default; every group is a tab.
    let html = page(&e, "/admin/settings", Some(&admin)).await;
    assert!(html.contains(r#"name="brand.name""#) && !html.contains("market.steady"));
    // An empty box shows the default it falls back on, greyed inside it,
    // and counts its characters.
    assert!(
        html.contains(r#"value="" placeholder="Agri-Connect""#),
        "{html}"
    );
    assert!(html.contains(r#"x-data="charCount" data-for="brand-name""#));
    assert!(html.contains(r#"href="/admin/settings?tab=market""#));
    assert!(html.contains("Đang theo mặc định"));
    let html = page(&e, "/admin/settings?tab=market", Some(&admin)).await;
    assert!(html.contains("Thị trường và đối tác") && html.contains("Liên hệ hỗ trợ"));
    assert!(html.contains(r#"name="market.steady_weekly_leads" type="number" value="20""#));
    assert!(html.contains("Mặc định 20, từ 1 đến 1000."));
    assert!(html.contains(r#"href="/admin/settings""#), "the menu entry");

    // Saved: back on its card, one row per value off its default.
    let res = save(
        &e,
        &admin,
        &[
            ("group", "market"),
            ("market.steady_weekly_leads", "8"),
            ("market.sponsored_shown", "2"),
            ("reason", "vùng thưa"),
        ],
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        res.headers()[header::LOCATION],
        "/admin/settings?tab=market"
    );
    assert_eq!(
        scalar(
            &e,
            "SELECT string_agg(key || '=' || value, ',') AS v FROM platform_settings"
        )
        .await,
        "market.steady_weekly_leads=8",
        "a value left at its default is not stored"
    );
    assert_eq!(
        scalar(
            &e,
            "SELECT reason AS v FROM audit_logs WHERE action = 'change_settings'"
        )
        .await,
        "market.steady_weekly_leads: 20 → 8: vùng thưa"
    );
    let html = page(&e, "/admin/settings?tab=market", Some(&admin)).await;
    assert!(
        html.contains("1 mục đã đổi"),
        "the card says how many are changed"
    );
    assert!(
        html.contains(r#"<span class="search-tab__badge">1</span>"#),
        "so does the tab"
    );
    assert!(
        html.contains("Đưa về mặc định") && html.contains("8 → 20"),
        "the reset card"
    );

    // Refused: the page again, the value as typed, the message under it.
    let res = save(
        &e,
        &admin,
        &[
            ("group", "jobs"),
            ("jobs.open_days", "0"),
            ("jobs.notify_max", "nhiều"),
        ],
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Nhập một số từ 1 đến 30."));
    assert!(html.contains("Nhập một số nguyên hợp lệ."));
    assert!(html.contains(r#"name="jobs.notify_max" type="number" value="nhiều""#));
    assert_eq!(
        scalar(&e, "SELECT count(*)::text AS v FROM platform_settings").await,
        "1",
        "nothing of a refused card is stored"
    );

    // Back on the defaults.
    let res = save(&e, &admin, &[("group", "market"), ("action", "reset")]).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        scalar(&e, "SELECT count(*)::text AS v FROM platform_settings").await,
        "0"
    );
    let html = page(&e, "/admin/settings?tab=market", Some(&admin)).await;
    assert!(!html.contains("mục đã đổi"));
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_json_api_takes_known_keys_and_admins_only() {
    let e = env_with_origin().await;
    install_admin(&e).await;
    let admin = bearer(&e, "quantri", "matkhau123").await;
    let provider = register_provider(&e.state, "vuonba", "0912000111").await;
    let patch = |token: &str, body: &str| {
        Request::patch("/api/admin/settings")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, TEST_ORIGIN)
            .body(Body::from(body.to_owned()))
            .unwrap()
    };

    let res = send(
        &e.router,
        patch(&admin, r#"{"values":{"jobs.notify_radius_km":"25"}}"#),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = send(
        &e.router,
        Request::get("/api/admin/settings")
            .header(header::AUTHORIZATION, format!("Bearer {admin}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let groups = json_body(res).await;
    let jobs = groups
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["code"] == "jobs")
        .unwrap();
    assert_eq!(jobs["changed"], 1);
    let radius = jobs["settings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "jobs.notify_radius_km")
        .unwrap();
    assert_eq!(
        (radius["value"].as_str(), radius["default"].as_str()),
        (Some("25"), Some("10"))
    );

    let res = send(&e.router, patch(&admin, r#"{"values":{"no.such":"1"}}"#)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["code"], "unknown_setting");
    let res = send(
        &e.router,
        patch(
            &admin,
            r#"{"values":{"support.zalo_url":"javascript:alert(1)"}}"#,
        ),
    )
    .await;
    let body = json_body(res).await;
    assert!(body["field_errors"]["support.zalo_url"][0]
        .as_str()
        .is_some_and(|m| m.contains("https://")));
    let res = send(
        &e.router,
        patch(&provider, r#"{"values":{"jobs.notify_max":"1"}}"#),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_name_an_admin_chooses_reaches_every_frame() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    assert!(page(&e, "/", None).await.contains("<title>Agri-Connect"));

    // One line, whatever was typed: a title and an email subject take no
    // line break.
    let res = save(
        &e,
        &admin,
        &[
            ("group", "brand"),
            ("brand.name", " Nông\r\nKết Nối "),
            ("brand.description", "Thuê làm, mua vật tư gần nhà."),
        ],
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        scalar(
            &e,
            "SELECT value AS v FROM platform_settings WHERE key = 'brand.name'"
        )
        .await,
        "Nông Kết Nối"
    );

    let home = page(&e, "/", None).await;
    assert!(home.contains("<title>Nông Kết Nối — "), "the page title");
    assert!(home.contains(r#"content="Thuê làm, mua vật tư gần nhà.""#));
    assert!(!home.contains("Agri-Connect"), "no trace of the old name");
    let login = page(&e, "/login", None).await;
    assert!(login.contains(r#"<span class="auth-brand__name">Nông Kết Nối</span>"#));
    let admin_page = page(&e, "/admin/settings", Some(&admin)).await;
    assert!(admin_page.contains("Nông Kết Nối Admin"), "the admin bar");
    let missing = send(&e.router, get_page("/no-such-page", None)).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert!(body_text(missing).await.contains("Nông Kết Nối"));

    // Too long: refused on its box, nothing stored.
    let long = "x".repeat(application::constants::SITE_NAME_MAX + 1);
    let res = save(&e, &admin, &[("group", "brand"), ("brand.name", &long)]).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains("Tối đa 40 ký tự."));

    // Back to defaults: the deployment's name again.
    let res = save(&e, &admin, &[("group", "brand"), ("action", "reset")]).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(page(&e, "/", None).await.contains("<title>Agri-Connect"));
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_help_lines_reach_every_footer() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let home = page(&e, "/", None).await;
    assert!(!home.contains("tel:0912345678"));

    let res = save(
        &e,
        &admin,
        &[
            ("group", "support"),
            ("support.phone", "0912 345 678"),
            ("support.zalo_url", "https://zalo.me/0912345678"),
            ("support.email", ""),
        ],
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let home = page(&e, "/", None).await;
    assert!(
        home.contains(r#"href="tel:0912345678""#),
        "stored without spaces"
    );
    assert!(home.contains(r#"href="https://zalo.me/0912345678""#));
    assert!(!home.contains("mailto:"), "an empty line is not shown");
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn closing_the_doors_refuses_sign_ups_and_job_posts() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let home = page(&e, "/", None).await;
    assert!(home.contains(r#"href="/register""#) && home.contains(r#"href="/jobs/new""#));

    // The sign-up switch unticked (the browser sends nothing), hire
    // requests off.
    let res = save(
        &e,
        &admin,
        &[("group", "features"), ("access.job_posts_open", "off")],
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let home = page(&e, "/", None).await;
    assert!(
        !home.contains(r#"href="/register""#),
        "no invitation to sign up"
    );
    assert!(
        !home.contains(r#"href="/jobs/new""#),
        "no way to the job form"
    );
    let register = page(&e, "/register", None).await;
    assert!(register.contains("Hiện tạm ngưng nhận đăng ký nhà cung cấp mới."));
    assert!(!register.contains(r#"name="business_name""#));
    let login = page(&e, "/login", None).await;
    assert!(!login.contains(r#"href="/register""#));
    let res = send(
        &e.router,
        Request::post("/api/users")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, TEST_ORIGIN)
            .body(Body::from(
                r#"{"username":"vuonmoi","email":"moi@example.vn","password":"matkhau123",
                    "business_name":"Vườn Mới","business_phone":"0912000222"}"#,
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(res).await["code"], "registration_closed");

    // Off, the job pages answer like pages that do not exist.
    let res = send(&e.router, get_page("/jobs/new", None)).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let category = categories_of(&e, "service").await[0].0;
    let area = scalar(&e, "SELECT min(id)::text AS v FROM locations").await;
    let res = send(
        &e.router,
        Request::post("/api/explore/jobs")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, TEST_ORIGIN)
            .body(Body::from(format!(
                r#"{{"category_id":{category},"location_id":{area},"description":"Phun thuoc",
                    "requester_name":"Chu Ba","requester_phone":"0912000333"}}"#
            )))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_body(res).await["code"], "route_not_found");

    // Open again: both ways back.
    let res = save(
        &e,
        &admin,
        &[
            ("group", "features"),
            ("access.registration_open", "true"),
            ("access.job_posts_open", "on"),
        ],
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let home = page(&e, "/", None).await;
    assert!(home.contains(r#"href="/register""#) && home.contains(r#"href="/jobs/new""#));
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_listing_cap_follows_its_setting() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let token = register_provider(&e.state, "vuonba", "0912000111").await;
    let res = save(
        &e,
        &admin,
        &[
            ("group", "listings"),
            ("listings.services_per_business", "1"),
            ("listings.products_per_business", "200"),
            ("listings.review_suspect_minutes", "60"),
        ],
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let category = categories_of(&e, "service").await[0].0;
    let add = || {
        Request::post("/api/provider/services")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, TEST_ORIGIN)
            .body(Body::from(format!(r#"{{"category_id":{category}}}"#)))
            .unwrap()
    };
    assert_eq!(send(&e.router, add()).await.status(), StatusCode::CREATED);
    let res = send(&e.router, add()).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = json_body(res).await;
    assert_eq!(body["code"], "service_limit_reached");
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_commodity_keeps_its_own_price_freshness() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let id = scalar(&e, "SELECT min(id)::text AS v FROM commodities").await;
    let (name, sector) = (
        scalar(
            &e,
            &format!("SELECT name AS v FROM commodities WHERE id = {id}"),
        )
        .await,
        scalar(
            &e,
            &format!("SELECT sector AS v FROM commodities WHERE id = {id}"),
        )
        .await,
    );
    let edit = |days: &str| {
        post_form(
            &format!("/admin/commodities/{id}"),
            &[
                ("name", name.as_str()),
                ("sector", sector.as_str()),
                ("display_order", "1"),
                ("price_fresh_days", days),
            ],
            Some(&admin),
        )
    };
    let html = page(&e, &format!("/admin/commodities/{id}"), Some(&admin)).await;
    assert!(html.contains(r#"name="price_fresh_days" type="number" value="14""#));

    assert_eq!(
        send(&e.router, edit("3")).await.status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        scalar(
            &e,
            &format!("SELECT price_fresh_days::text AS v FROM commodities WHERE id = {id}")
        )
        .await,
        "3"
    );
    let res = send(&e.router, edit("61")).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains("Nhập số ngày từ 1 đến 60."));
}

/// A mark on a transparent background, `w` × `h`.
fn logo_png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        let inside = x > w / 4 && x < 3 * w / 4 && y > h / 4 && y < 3 * h / 4;
        image::Rgba(if inside {
            [6, 78, 59, 255]
        } else {
            [0, 0, 0, 0]
        })
    });
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&img, w, h, image::ExtendedColorType::Rgba8)
        .unwrap();
    out
}

/// The identity tab's one form: its words and the files chosen, each a
/// (setting key, bytes) box; the boxes not given are sent empty.
async fn save_brand(
    e: &Env,
    admin: &str,
    fields: &[(&str, &str)],
    files: &[(&str, &[u8])],
) -> axum::http::Response<Body> {
    let boxes: Vec<(&str, Option<&[u8]>)> = ["brand.logo", "brand.icon"]
        .into_iter()
        .map(|key| (key, files.iter().find(|(k, _)| *k == key).map(|(_, b)| *b)))
        .collect();
    send(
        &e.router,
        post_multipart_files("/admin/settings/brand", fields, &boxes, admin),
    )
    .await
}

async fn stored_image(e: &Env, key: &str) -> String {
    scalar(
        e,
        &format!("SELECT coalesce(max(value), '') AS v FROM platform_settings WHERE key = '{key}'"),
    )
    .await
}

async fn file_count(e: &Env) -> String {
    scalar(e, "SELECT count(*)::text AS v FROM files").await
}

async fn audit_count(e: &Env) -> String {
    scalar(
        e,
        "SELECT count(*)::text AS v FROM audit_logs WHERE action = 'change_settings'",
    )
    .await
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_identity_form_saves_words_and_images_at_once_or_nothing() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let html = page(&e, "/admin/settings", Some(&admin)).await;
    assert!(html.contains(r#"action="/admin/settings/brand" enctype="multipart/form-data""#));
    assert!(
        html.contains(r#"name="brand.logo" type="file""#)
            && html.contains(r#"name="brand.icon" type="file""#)
    );
    assert!(html.contains(r#"x-data="brandPreview""#) && html.contains("Kết quả tìm kiếm Google"));

    // Every broken box at once, words and images; nothing stored.
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
    let long = "x".repeat(application::constants::SITE_NAME_MAX + 1);
    let res = save_brand(
        &e,
        &admin,
        &[("brand.name", &long), ("brand.description", "")],
        &[("brand.logo", svg), ("brand.icon", &logo_png(100, 100))],
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Tối đa 40 ký tự."), "the name");
    assert!(
        html.contains("Chỉ nhận ảnh JPEG, PNG hoặc WebP."),
        "the logo"
    );
    assert!(html.contains("Ảnh quá nhỏ."), "the icon");
    assert!(
        html.contains(&format!(r#"value="{long}""#)),
        "the name as typed"
    );
    assert_eq!(file_count(&e).await, "0");
    assert_eq!(stored_image(&e, "brand.name").await, "");

    // The same box twice would store a file nothing shows.
    let twice = post_multipart_files(
        "/admin/settings/brand",
        &[],
        &[
            ("brand.logo", Some(&logo_png(256, 256)[..])),
            ("brand.logo", Some(&logo_png(256, 256)[..])),
        ],
        &admin,
    );
    assert_eq!(
        send(&e.router, twice).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(file_count(&e).await, "0");

    // A typed file id is refused: it could point at any upload.
    let token = bearer(&e, "quantri", "matkhau123").await;
    let req = Request::patch("/api/admin/settings")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, TEST_ORIGIN)
        .body(Body::from(
            r#"{"values":{"brand.logo":"00000000-0000-0000-0000-000000000001"}}"#,
        ))
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    assert!(
        body["field_errors"]["brand.logo"][0]
            .as_str()
            .is_some_and(|m| m.contains("tải ảnh lên")),
        "{body}"
    );

    // One save: the name and the logo, one audit row, on every frame.
    let audits = audit_count(&e).await;
    let res = save_brand(
        &e,
        &admin,
        &[
            ("brand.name", "Nông Kết Nối"),
            ("brand.description", ""),
            ("reason", "nhận diện mới"),
        ],
        &[("brand.logo", &logo_png(300, 240))],
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/admin/settings?tab=brand");
    assert_eq!(
        audit_count(&e).await,
        (audits.parse::<i64>().unwrap() + 1).to_string()
    );
    let logo = stored_image(&e, "brand.logo").await;
    let thumb = format!("/files/{logo}/thumb");
    let home = page(&e, "/", None).await;
    assert!(home.contains(&format!(
        r#"class="brand-logo brand-logo--upload app-nav__logo" src="{thumb}""#
    )));
    assert!(home.contains(&format!(
        r#"<link rel="icon" href="{thumb}" type="image/png""#
    )));
    assert!(
        home.contains(r#"<span class="app-nav__name" title="Nông Kết Nối">Nông Kết Nối</span>"#)
    );
    assert!(page(&e, "/login", None).await.contains(&thumb));
    let res = send(&e.router, get_page(&format!("/files/{logo}"), None)).await;
    assert_eq!(res.headers()[header::CONTENT_TYPE], "image/png");
    let res = send(&e.router, get_page("/favicon.ico", None)).await;
    assert_eq!(res.status(), StatusCode::FOUND);
    assert_eq!(res.headers()[header::LOCATION], thumb.as_str());

    // A browser icon of its own; a new logo releases the old one. The words
    // sent again unchanged change nothing.
    let res = save_brand(
        &e,
        &admin,
        &[("brand.name", "Nông Kết Nối"), ("brand.description", "")],
        &[
            ("brand.icon", &logo_png(192, 192)),
            ("brand.logo", &logo_png(512, 512)),
        ],
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let icon = stored_image(&e, "brand.icon").await;
    let new_logo = stored_image(&e, "brand.logo").await;
    assert_ne!(new_logo, logo);
    assert_eq!(file_count(&e).await, "2", "the first logo is gone");
    let res = send(&e.router, get_page(&thumb, None)).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let home = page(&e, "/", None).await;
    assert!(home.contains(&format!(r#"<link rel="icon" href="/files/{icon}/thumb""#)));
    assert!(home.contains(&format!("/files/{new_logo}/thumb")));

    // Back to defaults: the words only; an upload is never deleted with a
    // whole group.
    let html = page(&e, "/admin/settings", Some(&admin)).await;
    assert!(html.contains("Logo và biểu tượng đã tải lên giữ nguyên"));
    let res = save(&e, &admin, &[("group", "brand"), ("action", "reset")]).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(stored_image(&e, "brand.name").await, "");
    assert_eq!(file_count(&e).await, "2");

    // Each image back on its default, behind its own confirmation.
    for key in ["brand.logo", "brand.icon"] {
        let res = send(
            &e.router,
            post_form(
                "/admin/settings/image/delete",
                &[("key", key), ("reason", "sai màu")],
                Some(&admin),
            ),
        )
        .await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
    }
    assert_eq!(file_count(&e).await, "0");
    let home = page(&e, "/", None).await;
    assert!(!home.contains("brand-logo--upload"), "the built-in mark");
    let res = send(&e.router, get_page("/favicon.ico", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
}
