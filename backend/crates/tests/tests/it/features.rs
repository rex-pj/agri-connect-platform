//! The features an admin opens to farmers (`/admin/settings?tab=features`,
//! `web::features`): off, a feature's pages answer like pages that do not
//! exist and no page links to them; in preview only an admin sees them,
//! never indexed nor cached; the JSON API says which are shown.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use application::features::Feature;
use axum::http::{header, Request, StatusCode};
use integration_tests::{
    body_text, create_product, env_with_origin, features_form, get_page, install_admin, json_body,
    post_form, register_provider, send, with_product_category, Env, TEST_ORIGIN,
};
use sea_orm::{ConnectionTrait, Statement};
use web::features::feature_of;

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

/// Sets `feature` to `stage` (`on`, `preview`, `off`) as the settings form
/// does; the sign-up switch stays ticked.
async fn set_stage(e: &Env, admin: &str, feature: Feature, stage: &str) {
    let res = send(
        &e.router,
        post_form(
            "/admin/settings",
            &features_form(&[
                ("access.registration_open", "true"),
                (feature.setting_key(), stage),
            ]),
            Some(admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "{}", feature.code());
}

/// Where `path` redirects to (a short address to its canonical one).
async fn canonical(e: &Env, path: &str) -> String {
    let res = send(&e.router, get_page(path, None)).await;
    assert_eq!(res.status(), StatusCode::MOVED_PERMANENTLY, "{path}");
    res.headers()[header::LOCATION].to_str().unwrap().to_owned()
}

/// A provider with a service and a supply, placed near a preset area; the
/// pages that show them.
struct Listing {
    provider: String,
    service: String,
    product: String,
    near: String,
}

async fn listing(e: &Env) -> Listing {
    let token = register_provider(&e.state, "vuonmot", "0912000444").await;
    let business = scalar(e, "SELECT min(id)::text AS v FROM businesses").await;
    e.t.db
        .execute_unprepared(&format!(
            "INSERT INTO services (business_id, service_type_id, name)
             SELECT {business}, id, 'Phun thuoc lua' FROM service_types
             ORDER BY display_order LIMIT 1"
        ))
        .await
        .unwrap();
    let product = create_product(
        e,
        &token,
        &with_product_category(e, r#"{"name":"Phan NPK","price":100000,"unit":"bao"}"#).await,
    )
    .await;
    let service = scalar(e, "SELECT min(id)::text AS v FROM services").await;
    let area = scalar(e, "SELECT min(id)::text AS v FROM locations").await;
    let (lat, lng) = {
        let row =
            e.t.db
                .query_one(Statement::from_string(
                    e.t.db.get_database_backend(),
                    format!("SELECT lat::float8 AS lat, lng::float8 AS lng FROM locations WHERE id = {area}"),
                ))
                .await
                .unwrap()
                .unwrap();
        (
            row.try_get::<f64>("", "lat").unwrap(),
            row.try_get::<f64>("", "lng").unwrap(),
        )
    };
    integration_tests::place_business(e, business.parse().unwrap(), lat, lng).await;
    Listing {
        provider: canonical(e, &format!("/providers/{business}")).await,
        service: canonical(e, &format!("/services/{service}")).await,
        product: canonical(e, &format!("/supplies/{product}")).await,
        near: format!("/explore?location_id={area}"),
    }
}

/// Every local link of a page, `&amp;` decoded.
fn links(html: &str) -> Vec<String> {
    html.split("href=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .filter(|href| href.starts_with('/'))
        .map(|href| href.replace("&amp;", "&"))
        .collect()
}

fn owned_by(href: &str, feature: Feature) -> bool {
    let (path, query) = href.split_once('?').unwrap_or((href, ""));
    let path = path.split('#').next().unwrap_or(path);
    feature_of(path, Some(query)) == Some(feature)
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_feature_off_is_gone_with_every_way_to_it() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let l = listing(&e).await;
    for feature in Feature::ALL {
        set_stage(&e, &admin, feature, "off").await;
        // Its own page, for a visitor and for an admin alike.
        if let Some(route) = web::features::page_route(feature) {
            let path = web::routes::url(route, &[]).unwrap();
            for cookie in [None, Some(admin.as_str())] {
                let res = send(&e.router, get_page(&path, cookie)).await;
                assert_eq!(res.status(), StatusCode::NOT_FOUND, "{path}");
                assert_eq!(res.headers()[header::CACHE_CONTROL], "private, no-cache");
                // The 404 page's own frame leads nowhere hidden either.
                let html = body_text(res).await;
                let bad: Vec<_> = links(&html)
                    .into_iter()
                    .filter(|h| owned_by(h, feature))
                    .collect();
                assert!(bad.is_empty(), "{path} (404) links to {bad:?}");
            }
        }
        // No page a visitor can open links to it.
        let mut pages = vec![
            "/".to_owned(),
            "/services".to_owned(),
            "/search?q=phun".to_owned(),
            "/search?q=khongcoai".to_owned(),
            l.provider.clone(),
            l.service.clone(),
        ];
        let others = [
            (Feature::NearMe, l.near.clone()),
            (Feature::Prices, "/prices".to_owned()),
            (Feature::Supplies, "/supplies".to_owned()),
            (Feature::Supplies, l.product.clone()),
            (Feature::Directory, "/providers".to_owned()),
            (Feature::Jobs, "/jobs/new".to_owned()),
        ];
        pages.extend(
            others
                .into_iter()
                .filter(|(f, _)| *f != feature)
                .map(|(_, p)| p),
        );
        for path in pages {
            let res = send(&e.router, get_page(&path, None)).await;
            assert_eq!(
                res.status(),
                StatusCode::OK,
                "{path} with {} off",
                feature.code()
            );
            let html = body_text(res).await;
            let bad: Vec<_> = links(&html)
                .into_iter()
                .filter(|h| owned_by(h, feature))
                .collect();
            assert!(
                bad.is_empty(),
                "{path} links to {bad:?} while {} is off",
                feature.code()
            );
        }
        set_stage(&e, &admin, feature, "on").await;
    }
    // Every feature back on: the ways in are there again.
    let home = body_text(send(&e.router, get_page("/", None)).await).await;
    for href in [
        r#"href="/prices""#,
        r#"href="/explore""#,
        r#"href="/supplies""#,
        r#"href="/jobs/new""#,
    ] {
        assert!(home.contains(href), "{href} on the home page");
    }
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_feature_in_preview_is_seen_by_admins_only() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    set_stage(&e, &admin, Feature::Prices, "preview").await;

    let res = send(&e.router, get_page("/prices", None)).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let home = body_text(send(&e.router, get_page("/", None)).await).await;
    assert!(!home.contains(r#"href="/prices""#));

    let res = send(&e.router, get_page("/prices", Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["x-robots-tag"], "noindex");
    assert_eq!(res.headers()[header::CACHE_CONTROL], "private, no-cache");
    let board = body_text(res).await;
    assert!(board.contains("Đang xem trước: bà con chưa thấy phần này."));
    assert!(board.contains(r#"href="/admin/settings?tab=features""#));
    let home = body_text(send(&e.router, get_page("/", Some(&admin))).await).await;
    assert!(home.contains(r#"href="/prices""#), "the admin's tab");
    assert!(
        !home.contains("Đang xem trước"),
        "the strip only on the feature's pages"
    );

    // The JSON API: the board is not there for a visitor, and says so.
    let res = send(&e.router, get_page("/api/explore/prices", None)).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let shown = json_body(send(&e.router, get_page("/api/explore/features", None)).await).await;
    assert_eq!(shown["prices"], false);
    assert_eq!(shown["near_me"], true);

    // The settings tab says where it stands and offers to try it.
    let tab = body_text(
        send(
            &e.router,
            get_page("/admin/settings?tab=features", Some(&admin)),
        )
        .await,
    )
    .await;
    assert!(
        tab.contains(r#"id="features-prices-preview" value="preview" autocomplete="off" checked"#)
    );
    assert!(tab.contains("Xem thử trang"));
    // The dashboard reminds of it.
    let dashboard =
        body_text(send(&e.router, get_page("/admin/dashboard", Some(&admin))).await).await;
    assert!(dashboard.contains("2 tính năng chưa mở cho bà con."));
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_stage_is_checked_and_a_write_of_a_closed_feature_refused() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let res = send(
        &e.router,
        post_form(
            "/admin/settings",
            &[("group", "features"), ("features.reviews", "maybe")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Supplies off: a provider lists none, by any way.
    let token = register_provider(&e.state, "vuonhai", "0912000555").await;
    set_stage(&e, &admin, Feature::Supplies, "off").await;
    let req = Request::post("/api/provider/products")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, TEST_ORIGIN)
        .body(axum::body::Body::from(
            with_product_category(&e, r#"{"name":"Phan NPK"}"#).await,
        ))
        .unwrap();
    assert_eq!(send(&e.router, req).await.status(), StatusCode::NOT_FOUND);
    let actor = e
        .state
        .usecases
        .auth
        .session_actor(&e.state.token_service.verify(&token).unwrap())
        .await
        .unwrap()
        .unwrap();
    let category = integration_tests::first_product_category(&e).await;
    let err = e
        .state
        .usecases
        .provider_dashboard
        .create_product(
            &actor,
            application::inputs::CreateProductInput {
                category_id: category,
                commodity_id: None,
                name: "Phan NPK".to_owned(),
                description: None,
                price: None,
                unit: None,
            },
            None,
        )
        .await
        .err()
        .map(|e| e.code().as_str());
    assert_eq!(err, Some("feature_off"));
}

/// The bars' and footer's links of a page: (wide bar, phone bar) tabs by
/// href as `bottom_nav` marks them, and the footer's browse column.
fn frame(html: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let bar = html
        .split("<nav class=\"bottom-nav\"")
        .nth(1)
        .and_then(|rest| rest.split("</nav>").next())
        .unwrap_or_default();
    let (mut top, mut phone) = (Vec::new(), Vec::new());
    for tab in bar.split("<a class=\"bottom-nav__tab").skip(1) {
        let classes = tab.split('"').next().unwrap_or_default();
        let href = tab
            .split("href=\"")
            .nth(1)
            .and_then(|h| h.split('"').next())
            .unwrap_or_default()
            .to_owned();
        if !classes.contains("d-none d-xl-flex") {
            phone.push(href.clone());
        }
        if !classes.contains("d-xl-none") {
            top.push(href);
        }
    }
    let footer = html
        .split("id=\"footer-explore\"")
        .nth(1)
        .and_then(|rest| rest.split("</nav>").next())
        .map(links)
        .unwrap_or_default();
    (top, phone, footer)
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_admin_chooses_where_each_feature_is_led_to() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let page = |cookie: Option<String>| {
        let router = e.router.clone();
        async move { body_text(send(&router, get_page("/", cookie.as_deref())).await).await }
    };

    // As it was: near me and supplies everywhere, prices and the directory
    // on the wide bar and in the footer, hire requests nowhere.
    let (top, phone, footer) = frame(&page(None).await);
    for href in ["/explore", "/supplies", "/providers", "/prices"] {
        assert!(top.iter().any(|h| h == href), "{href} on the wide bar");
        assert!(footer.iter().any(|h| h == href), "{href} in the footer");
    }
    assert!(phone.iter().any(|h| h == "/explore") && !phone.iter().any(|h| h == "/prices"));
    assert!(!top.iter().any(|h| h == "/jobs/new"));

    // Prices on the phone instead of supplies, out of the footer; hire
    // requests in the footer.
    let res = send(
        &e.router,
        post_form(
            "/admin/settings",
            &features_form(&[
                ("access.registration_open", "true"),
                ("nav.supplies.phone", "false"),
                ("nav.prices.phone", "true"),
                ("nav.prices.footer", "false"),
                ("nav.jobs.footer", "true"),
            ]),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let (top, phone, footer) = frame(&page(None).await);
    assert!(phone.iter().any(|h| h == "/prices"), "{phone:?}");
    assert!(!phone.iter().any(|h| h == "/supplies"), "{phone:?}");
    assert!(
        top.iter().any(|h| h == "/supplies"),
        "still on the wide bar"
    );
    assert!(!footer.iter().any(|h| h == "/prices"), "{footer:?}");
    assert!(footer.iter().any(|h| h == "/jobs/new"), "{footer:?}");

    // A place never shows a feature that is off.
    set_stage(&e, &admin, Feature::Jobs, "off").await;
    let (_, _, footer) = frame(&page(None).await);
    assert!(!footer.iter().any(|h| h == "/jobs/new"));

    // A third phone tab is refused, said under the box.
    let res = send(
        &e.router,
        post_form(
            "/admin/settings",
            &features_form(&[
                ("access.registration_open", "true"),
                ("nav.directory.phone", "true"),
            ]),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let form = body_text(res).await;
    assert!(form.contains("chỉ có chỗ cho 2 tính năng"), "the message");

    // So is a fifth feature on the wide bar, once it shows (hire requests
    // are off here: their tick alone would take no room).
    let res = send(
        &e.router,
        post_form(
            "/admin/settings",
            &features_form(&[
                ("access.registration_open", "true"),
                ("access.job_posts_open", "on"),
                ("nav.jobs.top", "true"),
            ]),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res)
        .await
        .contains("Thanh trên chỉ có chỗ cho 4 tính năng"));

    // The tab shows the boxes and the preview of the menus.
    let tab = body_text(
        send(
            &e.router,
            get_page("/admin/settings?tab=features", Some(&admin)),
        )
        .await,
    )
    .await;
    // (`set_stage` sent the form back on its default places.)
    assert!(tab.contains(r#"name="nav.supplies.phone" value="true" checked data-room="phone""#));
    assert!(tab.contains(r#"name="nav.prices.phone" value="true" data-room="phone""#));
    assert!(tab.contains(r#"data-place-key="nav.jobs.footer""#));
    assert!(tab.contains("2/2 chỗ cho tính năng"));
    // Hire requests are off: their boxes are kept, faded, and said to count
    // for nothing yet.
    assert!(tab
        .contains(r#"class="feature-row__places is-off" data-places-of="access.job_posts_open""#));
}

/// Each stage and box as the features tab shows it, the way the browser
/// sends the form back: every ticked box, every stage.
async fn tab_as_sent(e: &Env, admin: &str) -> Vec<(String, String)> {
    let html = body_text(
        send(
            &e.router,
            get_page("/admin/settings?tab=features", Some(admin)),
        )
        .await,
    )
    .await;
    let mut form = vec![("group".to_owned(), "features".to_owned())];
    for input in html.split("<input ").skip(1) {
        let tag = input.split('>').next().unwrap_or_default();
        let attr = |name: &str| {
            tag.split(&format!("{name}=\""))
                .nth(1)
                .and_then(|v| v.split('"').next())
                .map(str::to_owned)
        };
        let (Some(name), Some(value)) = (attr("name"), attr("value")) else {
            continue;
        };
        let checkable = tag.contains("type=\"checkbox\"") || tag.contains("type=\"radio\"");
        if (name.starts_with("nav.")
            || name.starts_with("features.")
            || name.starts_with("access."))
            && (!checkable || tag.contains(" checked"))
        {
            form.push((name, value));
        }
    }
    form
}

fn with(form: &[(String, String)], key: &str, value: &str) -> Vec<(String, String)> {
    let mut form: Vec<_> = form.iter().filter(|(k, _)| k != key).cloned().collect();
    if value != "false" {
        form.push((key.to_owned(), value.to_owned()));
    }
    form
}

async fn save_form(e: &Env, admin: &str, form: &[(String, String)]) -> StatusCode {
    let pairs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    send(&e.router, post_form("/admin/settings", &pairs, Some(admin)))
        .await
        .status()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_off_feature_never_blocks_the_tab_and_comes_back_on() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    // As a real database stood: prices and near me off, their top boxes
    // still ticked, hire requests added to the top bar: five ticks there.
    e.t.db
        .execute_unprepared(
            "INSERT INTO platform_settings (key, value, updated_on) VALUES
             ('features.prices', 'off', now()), ('features.near_me', 'off', now()),
             ('nav.jobs.top', 'true', now())",
        )
        .await
        .unwrap();
    let shown = |e: &Env| {
        let router = e.router.clone();
        async move { json_body(send(&router, get_page("/api/explore/features", None)).await).await }
    };

    // Turning near me back on, the form as the page sends it, is saved.
    let form = tab_as_sent(&e, &admin).await;
    let res = save_form(&e, &admin, &with(&form, "features.near_me", "on")).await;
    assert_eq!(res, StatusCode::SEE_OTHER, "near me back on");
    assert_eq!(shown(&e).await["near_me"], true);

    // An off feature's tick takes no room: hire requests' footer box and
    // anything else unrelated save as well.
    let form = tab_as_sent(&e, &admin).await;
    let res = save_form(&e, &admin, &with(&form, "nav.prices.phone", "true")).await;
    assert_eq!(res, StatusCode::SEE_OTHER, "an off feature's tick is kept");

    // Prices back on would make five on the top bar: refused on prices
    // itself, said why; unticking one of the others in the same save lets it.
    let form = tab_as_sent(&e, &admin).await;
    let res = send(
        &e.router,
        post_form(
            "/admin/settings",
            &with(&form, "features.prices", "on")
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect::<Vec<_>>(),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let refused = body_text(res).await;
    assert!(refused.contains("Thanh trên chỉ có chỗ cho 4 tính năng"));
    // The menus shown with the form as sent say the bar is over.
    assert!(refused.contains("5/4 chỗ: quá chỗ"), "the room line warns");
    assert_eq!(shown(&e).await["prices"], false);
    let fixed = with(
        &with(&form, "features.prices", "on"),
        "nav.jobs.top",
        "false",
    );
    // Prices also takes a phone tab now: make room there too.
    let fixed = with(&fixed, "nav.supplies.phone", "false");
    assert_eq!(save_form(&e, &admin, &fixed).await, StatusCode::SEE_OTHER);
    assert_eq!(shown(&e).await["prices"], true);

    // Off and on again, with nothing else touched, always round-trips.
    for feature in Feature::ALL {
        let key = feature.setting_key();
        let form = tab_as_sent(&e, &admin).await;
        assert_eq!(
            save_form(&e, &admin, &with(&form, key, "off")).await,
            StatusCode::SEE_OTHER,
            "{key} off"
        );
        assert_eq!(shown(&e).await[feature.code()], false, "{key} off");
        let form = tab_as_sent(&e, &admin).await;
        assert_eq!(
            save_form(&e, &admin, &with(&form, key, "on")).await,
            StatusCode::SEE_OTHER,
            "{key} on"
        );
        assert_eq!(shown(&e).await[feature.code()], true, "{key} on");
    }
}
