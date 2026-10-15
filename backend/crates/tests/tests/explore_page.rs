//! /explore: the near-me page. Covers the PostGIS search end to end (it had
//! no test before, and its custom SQL never bound its parameters), the preset
//! form that works without JS, and filter links that keep the position.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use integration_tests::{categories_of, json_body, register_provider, send, Env};
use sea_orm::{ConnectionTrait, Statement};

/// The "Cần Thơ" location preset from the catalogue.
const CAN_THO: (f64, f64) = (10.0452, 105.7469);
/// About one kilometre of latitude.
const KM_LAT: f64 = 0.009;

async fn html(e: &Env, uri: &str) -> String {
    let req = Request::get(uri)
        .header(header::ACCEPT, "text/html")
        .body(Body::empty())
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::OK, "{uri}");
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn names(html: &str) -> Vec<String> {
    html.split(r#"<h3 class="card-title fs-6 fw-bold mb-0 flex-grow-1">"#)
        .skip(1)
        .filter_map(|s| s.split_once("</h3>").map(|(n, _)| n.to_owned()))
        .collect()
}

async fn can_tho_id(e: &Env) -> i32 {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            "SELECT id FROM locations WHERE label = 'Cần Thơ'",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap()
}

/// Providers 1, 8 and 20 km north of Cần Thơ, each with a service in the
/// first service category. Returns that category id.
async fn seeded(e: &Env) -> i32 {
    let service = categories_of(e, "service").await[0].0;
    for (name, km) in [("Gần 1 km", 1.0), ("Vừa 8 km", 8.0), ("Xa 20 km", 20.0)] {
        let lat = CAN_THO.0 + km * KM_LAT;
        e.t.db
            .execute_unprepared(&format!(
                "WITH b AS (
                     INSERT INTO businesses (name, phone, address, location)
                     VALUES ('{name}', '0901234567', 'Ninh Kiều, Cần Thơ',
                             ST_GeographyFromText('SRID=4326;POINT({lng} {lat})'))
                     RETURNING id)
                 INSERT INTO services (business_id, category_id, name)
                 SELECT id, {service}, 'Dịch vụ' FROM b",
                lng = CAN_THO.1
            ))
            .await
            .unwrap();
    }
    service
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn without_a_position_the_page_offers_gps_and_a_preset_form() {
    let e = Env::new().await;
    let page = html(&e, "/explore").await;
    assert!(page.contains("<title>Tìm dịch vụ gần bạn — Agri-Connect</title>"));
    assert!(
        page.contains(r#"<ac-geolocate href="/explore""#),
        "the GPS island"
    );
    assert!(
        page.contains(r#"src="/static/js/islands.js?v="#),
        "island script, versioned"
    );
    // The form that works without JS.
    assert!(page.contains(r#"<form class="loc-manual" method="get" action="/explore">"#));
    assert!(page.contains(">Cần Thơ</option>"));
    // Service tiles lead to the service pages until there is a position.
    assert!(page.contains(r#"href="/services/category/"#));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_preset_finds_providers_by_distance_and_the_links_keep_it() {
    let e = Env::new().await;
    let service = seeded(&e).await;
    let id = can_tho_id(&e).await;

    let page = html(&e, &format!("/explore?location_id={id}")).await;
    // Default 10 km: the 20 km provider is out; nearest first.
    assert_eq!(names(&page), ["Gần 1 km", "Vừa 8 km"]);
    // Under a kilometre in metres, beyond it in km with the ride time.
    assert!(page.contains("Cách bạn <strong>995 m</strong>"));
    assert!(page.contains("Cách bạn <strong>8.0 km (~16 phút xe máy)</strong>"));
    assert!(
        page.contains("Cần Thơ</p>"),
        "the preset's name in the hero"
    );
    // Radius and service links keep the position.
    assert!(page.contains(&format!(
        r#"href="/explore?location_id={id}&amp;radius=25000""#
    )));
    let filtered = format!(r#"href="/explore?location_id={id}&amp;service_id={service}""#);
    assert!(page.contains(&filtered));

    let wide = html(&e, &format!("/explore?location_id={id}&radius=25000")).await;
    assert_eq!(names(&wide).len(), 3);

    let narrow = html(&e, &format!("/explore?location_id={id}&radius=5000")).await;
    assert_eq!(names(&narrow), ["Gần 1 km"]);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn device_coordinates_search_and_the_island_keeps_the_filters() {
    let e = Env::new().await;
    let service = seeded(&e).await;
    let uri = format!(
        "/explore?lat={}&lng={}&service_id={service}&radius=50000",
        CAN_THO.0, CAN_THO.1
    );
    let page = html(&e, &uri).await;
    assert_eq!(names(&page).len(), 3);
    assert!(page.contains("Đã xác định vị trí"));
    // "Cập nhật" re-locates without losing the radius or the service.
    assert!(page.contains(&format!(
        r#"<ac-geolocate href="/explore?lat={}&amp;lng={}&amp;radius=50000&amp;service_id={service}""#,
        CAN_THO.0, CAN_THO.1
    )));
    // The active service tile clears the filter.
    assert!(page.contains(&format!(
        r#"href="/explore?lat={}&amp;lng={}&amp;radius=50000" aria-current="page""#,
        CAN_THO.0, CAN_THO.1
    )));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn bad_positions_fall_back_to_the_onboarding_and_empty_results_offer_more_range() {
    let e = Env::new().await;
    seeded(&e).await;
    for uri in [
        "/explore?lat=abc&lng=105",
        "/explore?lat=95&lng=105",
        "/explore?location_id=999",
    ] {
        let page = html(&e, uri).await;
        assert!(page.contains("Tìm thợ gần bạn"), "{uri}");
    }

    // Far from everyone (Hà Nội): nothing within 10 km, offer 25 km.
    let page = html(&e, "/explore?lat=21.0278&lng=105.8342").await;
    assert!(page.contains("Không tìm thấy dịch vụ trong bán kính 10 km"));
    assert!(page.contains(r#"href="/explore?lat=21.0278&amp;lng=105.8342&amp;radius=25000""#));
    assert!(page.contains("Tìm trong 25 km"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_provider_saving_coordinates_appears_in_the_geo_api() {
    // update_location used an unbound `?` too: saving a GPS position failed.
    let e = Env::new().await;
    let service = categories_of(&e, "service").await[0].0;
    let token = register_provider(&e.state, "nongdan", "0901234567").await;
    e.t.db
        .execute_unprepared(&format!(
            "INSERT INTO services (business_id, category_id, name)
             SELECT id, {service}, 'Múc bùn' FROM businesses WHERE name = 'Vườn nongdan'"
        ))
        .await
        .unwrap();
    let req = Request::builder()
        .method(Method::PUT)
        .uri("/api/provider/profile")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!(
            r#"{{"name":"Vườn nongdan","phone":"0901234567","lat":{},"lng":{}}}"#,
            CAN_THO.0, CAN_THO.1
        )))
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::OK);

    let req = Request::get(format!(
        "/api/explore/providers?lat={}&lng={}&radius=1000",
        CAN_THO.0, CAN_THO.1
    ))
    .body(Body::empty())
    .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let found = json_body(res).await;
    assert_eq!(found[0]["name"], "Vườn nongdan", "{found}");
    assert!(found[0]["distance_m"].as_f64().unwrap() < 1.0);
    e.cleanup().await;
}
