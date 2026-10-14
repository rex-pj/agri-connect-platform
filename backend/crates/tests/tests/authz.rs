//! Authorization end to end: real database, real router, real tokens.
//! Run with TEST_DATABASE_URL and `--include-ignored` (see database.rs).

use application::catalogue::CATALOGUE;
use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    Router,
};
use domain::repositories::catalogue_repository_trait::CatalogueRepositoryTrait;
use infrastructure::catalogue_repository::CatalogueRepository;
use integration_tests::{first_admin, json_body, router_with, send, state, TestDb};
use sea_orm::ConnectionTrait;
use std::sync::Arc;
use web::{app_state::AppState, routings::app_routing::RouterConfig};

const ORIGIN: &str = "http://localhost:3000";

struct Env {
    t: TestDb,
    state: AppState,
    router: Router,
}

async fn env() -> Env {
    let t = TestDb::create().await;
    CatalogueRepository::new(Arc::new(t.db.clone()))
        .apply(&CATALOGUE)
        .await
        .unwrap();
    let state = state(t.db.clone());
    let router = router_with(
        state.clone(),
        RouterConfig {
            allowed_origins: vec![ORIGIN.to_owned()],
            ..RouterConfig::default()
        },
    );
    Env { t, state, router }
}

/// A token for a new provider, from the real registration flow.
async fn provider_token(state: &AppState) -> String {
    state
        .usecases
        .auth
        .register(
            "nongdan".into(),
            "nongdan@example.vn".into(),
            "matkhau123".into(),
            None,
            "Vườn Thử".into(),
            "0901234567".into(),
        )
        .await
        .unwrap_or_else(|e| panic!("register: {e}"))
        .token
}

async fn admin_token(state: &AppState) -> String {
    let admin = first_admin(state).await;
    state
        .usecases
        .auth
        .login(admin.username.clone(), "matkhau123".into())
        .await
        .unwrap_or_else(|e| panic!("login: {e}"))
        .token
}

fn get(uri: &str, token: &str) -> Request<Body> {
    Request::get(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn json(method: Method, uri: &str, token: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn code(res: axum::http::Response<Body>) -> String {
    json_body(res).await["code"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_provider_cannot_use_admin_endpoints() {
    let e = env().await;
    let token = provider_token(&e.state).await;
    let res = send(&e.router, get("/api/admin/stats", &token)).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(code(res).await, "permission_denied");
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_admin_can_and_a_revocation_applies_on_the_next_request() {
    let e = env().await;
    let token = admin_token(&e.state).await;
    assert_eq!(
        send(&e.router, get("/api/admin/stats", &token))
            .await
            .status(),
        StatusCode::OK
    );

    // Same token, no re-login: permissions are read per request.
    e.t.db
        .execute_unprepared(
            "DELETE FROM role_permissions WHERE permission_id =
               (SELECT id FROM permissions WHERE code = 'admin.dashboard.view')",
        )
        .await
        .unwrap();
    let res = send(&e.router, get("/api/admin/stats", &token)).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_deactivated_account_loses_its_session_at_once() {
    let e = env().await;
    let token = provider_token(&e.state).await;
    assert_eq!(
        send(&e.router, get("/api/users/me", &token)).await.status(),
        StatusCode::OK
    );

    e.t.db
        .execute_unprepared("UPDATE users SET is_active = FALSE WHERE username = 'nongdan'")
        .await
        .unwrap();
    let res = send(&e.router, get("/api/users/me", &token)).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(code(res).await, "authentication_required");
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_session_cookie_works_like_a_bearer_token() {
    let e = env().await;
    let token = provider_token(&e.state).await;
    let req = Request::get("/api/users/me")
        .header(header::COOKIE, format!("lang=vi; auth_token={token}"))
        .body(Body::empty())
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["username"], "nongdan");
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_admin_cannot_lock_themselves_out() {
    let e = env().await;
    let admin = first_admin(&e.state).await;
    let token = e
        .state
        .usecases
        .auth
        .login(admin.username.clone(), "matkhau123".into())
        .await
        .unwrap_or_else(|err| panic!("{err}"))
        .token;
    let uri = format!("/api/admin/users/{}/status", admin.user_id);
    let res = send(
        &e.router,
        json(Method::PATCH, &uri, &token, r#"{"is_active":false}"#),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(code(res).await, "cannot_change_own_account");

    let uri = format!("/api/admin/users/{}/role", admin.user_id);
    let res = send(
        &e.router,
        json(Method::PATCH, &uri, &token, r#"{"role_code":"provider"}"#),
    )
    .await;
    assert_eq!(code(res).await, "cannot_change_own_account");
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_cross_site_write_is_refused_and_ours_is_accepted() {
    let e = env().await;
    let token = admin_token(&e.state).await;
    let category = r#"{"is_active":false,"reason":"trùng"}"#;

    let mut evil = json(
        Method::PATCH,
        "/api/admin/categories/1/status",
        &token,
        category,
    );
    evil.headers_mut()
        .insert(header::ORIGIN, "https://evil.example".parse().unwrap());
    let res = send(&e.router, evil).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(code(res).await, "cross_origin_request");

    let mut ours = json(
        Method::PATCH,
        "/api/admin/categories/1/status",
        &token,
        category,
    );
    ours.headers_mut()
        .insert(header::ORIGIN, ORIGIN.parse().unwrap());
    assert_eq!(send(&e.router, ours).await.status(), StatusCode::OK);
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn admin_changes_reach_the_audit_log_through_events() {
    let e = env().await;
    let token = admin_token(&e.state).await;
    let res = send(
        &e.router,
        json(
            Method::PATCH,
            "/api/admin/categories/1/status",
            &token,
            r#"{"is_active":false,"reason":"trùng lặp"}"#,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    #[derive(sea_orm::FromQueryResult)]
    struct Row {
        action: String,
        entity_type: String,
        reason: Option<String>,
    }
    let rows =
        <Row as sea_orm::FromQueryResult>::find_by_statement(sea_orm::Statement::from_string(
            e.t.db.get_database_backend(),
            "SELECT action, entity_type, reason FROM audit_logs",
        ))
        .all(&e.t.db)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(
        (row.action.as_str(), row.entity_type.as_str()),
        ("hide_category", "category")
    );
    assert_eq!(row.reason.as_deref(), Some("trùng lặp"));
    e.t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_provider_with_a_revoked_write_permission_can_still_read() {
    let e = env().await;
    let token = provider_token(&e.state).await;
    e.t.db
        .execute_unprepared(
            "DELETE FROM role_permissions WHERE permission_id =
               (SELECT id FROM permissions WHERE code = 'own.listing.create')",
        )
        .await
        .unwrap();

    let res = send(
        &e.router,
        json(
            Method::POST,
            "/api/provider/products",
            &token,
            r#"{"category_id":4,"name":"Gạo"}"#,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        send(&e.router, get("/api/provider/products", &token))
            .await
            .status(),
        StatusCode::OK
    );
    e.t.cleanup().await;
}
