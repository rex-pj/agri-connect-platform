//! The features an admin opens to farmers (`/admin/settings?tab=features`,
//! `web::features`): off, a feature's pages answer like pages that do not
//! exist and no page links to them; in preview only an admin sees them,
//! never indexed nor cached; the JSON API says which are shown.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use application::features::Feature;
use axum::http::{header, Request, StatusCode};
use integration_tests::{
    body_text, create_product, env_with_origin, get_page, install_admin, json_body, post_form,
    register_provider, send, with_product_category, Env, TEST_ORIGIN,
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
            &[
                ("group", "features"),
                ("access.registration_open", "true"),
                (feature.setting_key(), stage),
            ],
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
    assert!(dashboard.contains("1 tính năng chưa mở cho bà con."));
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
