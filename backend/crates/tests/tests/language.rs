//! The language switch through the real router: the cookie it sets, where it
//! goes back to, and pages, error pages and the JSON API answering in the
//! chosen language. None of it touches the database.

use axum::{
    body::Body,
    http::{header, Request, Response, StatusCode},
    Router,
};
use integration_tests::{
    body_text, get_page, json_body, post_form, router_with, send, state, TEST_ORIGIN,
};
use sea_orm::DatabaseConnection;
use web::routings::app_routing::RouterConfig;

fn router() -> Router {
    router_with(
        state(DatabaseConnection::default()),
        RouterConfig {
            allowed_origins: vec![TEST_ORIGIN.to_owned()],
            ..RouterConfig::default()
        },
    )
}

fn location(res: &Response<Body>) -> &str {
    res.headers()[header::LOCATION].to_str().unwrap()
}

#[tokio::test]
async fn the_switch_keeps_the_choice_and_goes_back_to_the_page() {
    let res = send(
        &router(),
        post_form(
            "/language",
            &[("lang", "en"), ("back", "/products?q=g")],
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&res), "/products?q=g");
    let cookie = res.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(
        cookie.starts_with("lang=en;") && cookie.contains("SameSite=Lax"),
        "{cookie}"
    );
}

#[tokio::test]
async fn the_switch_never_sends_anyone_to_another_site() {
    for back in [
        "//evil.example/x",
        "https://evil.example",
        "/\\evil.example",
        "",
    ] {
        let res = send(
            &router(),
            post_form("/language", &[("lang", "vi"), ("back", back)], None),
        )
        .await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER, "{back:?}");
        assert_eq!(location(&res), "/", "{back:?}");
    }
}

#[tokio::test]
async fn an_unknown_language_is_refused() {
    let res = send(
        &router(),
        post_form("/language", &[("lang", "fr"), ("back", "/")], None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn pages_are_vietnamese_until_english_is_chosen() {
    let vi = body_text(send(&router(), get_page("/no-such-page", None)).await).await;
    assert!(vi.contains(r#"<html lang="vi">"#));
    assert!(vi.contains("Không tìm thấy địa chỉ này."));

    let res = send(&router(), get_page("/no-such-page", Some("lang=en"))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let en = body_text(res).await;
    assert!(en.contains(r#"<html lang="en">"#));
    assert!(en.contains("This page does not exist."));
    assert!(en.contains("Back to home"), "the frame is English too");
    // The switch shows English as the current choice and comes back here.
    assert!(en.contains(r#"value="en" lang="en" aria-pressed="true""#));
    assert!(en.contains(r#"name="back" value="/no-such-page""#));
}

#[tokio::test]
async fn the_json_api_answers_in_the_chosen_language() {
    let req = Request::get("/api/does-not-exist")
        .header(header::COOKIE, "lang=en")
        .body(Body::empty())
        .unwrap();
    let body = json_body(send(&router(), req).await).await;
    assert_eq!(body["code"], "route_not_found");
    assert_eq!(body["message"], "This page does not exist.");
}

#[tokio::test]
async fn a_cookie_naming_no_known_language_means_vietnamese() {
    let page = body_text(send(&router(), get_page("/no-such-page", Some("lang=xx"))).await).await;
    assert!(page.contains(r#"<html lang="vi">"#));
}
