//! Browser sessions: refresh-token rotation and reuse detection, renewal by
//! the middleware, logout. Database tests need TEST_DATABASE_URL and
//! `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, Response, StatusCode},
};
use integration_tests::{register_provider, send, Env};
use sea_orm::{ConnectionTrait, Statement};

async fn login(e: &Env) -> (i32, application::usecases::auth_usecase::SessionTokens) {
    register_provider(&e.state, "nongdan", "0901234567").await;
    let login = e
        .state
        .usecases
        .auth
        .login("nongdan".into(), "matkhau123".into())
        .await
        .unwrap();
    let tokens = e
        .state
        .usecases
        .auth
        .start_session(login.user_id)
        .await
        .unwrap();
    (login.user_id, tokens)
}

fn set_cookies(res: &Response<Body>) -> Vec<String> {
    res.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_owned())
        .collect()
}

fn cookie_value(set: &[String], name: &str) -> Option<String> {
    set.iter()
        .find_map(|c| c.strip_prefix(&format!("{name}=")))
        .and_then(|rest| rest.split(';').next())
        .map(str::to_owned)
}

async fn age_used_tokens(e: &Env, interval: &str) {
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE refresh_tokens SET used_at = now() - interval '{interval}' WHERE used_at IS NOT NULL"
        ))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_refresh_token_rotates_and_works_once() {
    let e = Env::new().await;
    let (_, first) = login(&e).await;
    let auth = &e.state.usecases.auth;

    let second = auth.refresh_session(&first.refresh).await.unwrap().unwrap();
    assert_ne!(second.refresh, first.refresh);
    assert!(e.state.token_service.verify(&second.access).is_ok());

    // The same token again, a moment later: two tabs refreshing together.
    // Refused, but the family survives, so the other tab stays signed in.
    assert_eq!(auth.refresh_session(&first.refresh).await.unwrap(), None);
    let third = auth.refresh_session(&second.refresh).await.unwrap();
    assert!(third.is_some(), "the winner's token still works");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn reusing_a_spent_token_later_revokes_the_whole_family() {
    let e = Env::new().await;
    let (_, first) = login(&e).await;
    let auth = &e.state.usecases.auth;
    let second = auth.refresh_session(&first.refresh).await.unwrap().unwrap();

    // Long after the rotation, someone presents the old token: a copy exists.
    age_used_tokens(&e, "5 minutes").await;
    assert_eq!(auth.refresh_session(&first.refresh).await.unwrap(), None);
    assert_eq!(
        auth.refresh_session(&second.refresh).await.unwrap(),
        None,
        "the legitimate holder must log in again too"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn only_hashes_are_stored() {
    let e = Env::new().await;
    let (_, tokens) = login(&e).await;
    let row =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                "SELECT token_hash FROM refresh_tokens",
            ))
            .await
            .unwrap()
            .unwrap();
    let stored: String = row.try_get("", "token_hash").unwrap();
    assert_ne!(stored, tokens.refresh);
    assert_eq!(stored.len(), 64, "hex SHA-256");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_middleware_renews_an_expired_session_for_the_same_request() {
    let e = Env::new().await;
    let (_, tokens) = login(&e).await;

    // No access cookie (expired and dropped by the browser), only the refresh.
    let req = Request::get("/api/users/me")
        .header(
            header::COOKIE,
            format!("lang=vi; refresh_token={}", tokens.refresh),
        )
        .body(Body::empty())
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "the handler saw the renewed session"
    );
    let set = set_cookies(&res);
    let access = cookie_value(&set, "auth_token").expect("new access cookie");
    let refresh = cookie_value(&set, "refresh_token").expect("rotated refresh cookie");
    assert!(e.state.token_service.verify(&access).is_ok());
    assert_ne!(refresh, tokens.refresh);
    assert!(set
        .iter()
        .all(|c| c.contains("SameSite=Lax") && c.contains("HttpOnly")));

    // A spent or unknown refresh token: signed out, and the dead cookies go.
    let req = Request::get("/api/users/me")
        .header(header::COOKIE, "refresh_token=khong-hop-le")
        .body(Body::empty())
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert!(set_cookies(&res).iter().all(|c| c.contains("Max-Age=0")));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_valid_access_token_or_a_bearer_header_never_rotates() {
    let e = Env::new().await;
    let (_, tokens) = login(&e).await;
    let both = format!(
        "auth_token={}; refresh_token={}",
        tokens.access, tokens.refresh
    );
    let req = Request::get("/api/users/me")
        .header(header::COOKIE, both)
        .body(Body::empty())
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(set_cookies(&res).is_empty(), "nothing to renew");

    let req = Request::get("/api/users/me")
        .header(header::AUTHORIZATION, format!("Bearer {}", tokens.access))
        .header(header::COOKIE, format!("refresh_token={}", tokens.refresh))
        .body(Body::empty())
        .unwrap();
    assert!(set_cookies(&send(&e.router, req).await).is_empty());
    // The refresh token is still unspent.
    assert!(e
        .state
        .usecases
        .auth
        .refresh_session(&tokens.refresh)
        .await
        .unwrap()
        .is_some());
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn logout_ends_the_session_and_clears_both_cookies() {
    let e = Env::new().await;
    let (_, tokens) = login(&e).await;
    for (path, home) in [("/logout", "/"), ("/admin/logout", "/login")] {
        let tokens = if path == "/logout" {
            tokens.clone()
        } else {
            let login = e
                .state
                .usecases
                .auth
                .login("nongdan".into(), "matkhau123".into())
                .await
                .unwrap();
            e.state
                .usecases
                .auth
                .start_session(login.user_id)
                .await
                .unwrap()
        };
        let req = Request::post(path)
            .header(
                header::COOKIE,
                format!(
                    "auth_token={}; refresh_token={}",
                    tokens.access, tokens.refresh
                ),
            )
            .body(Body::empty())
            .unwrap();
        let res = send(&e.router, req).await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER, "{path}");
        assert_eq!(res.headers()[header::LOCATION], home);
        let set = set_cookies(&res);
        assert_eq!(set.len(), 2);
        assert!(set.iter().all(|c| c.contains("Max-Age=0")));
        assert_eq!(
            e.state
                .usecases
                .auth
                .refresh_session(&tokens.refresh)
                .await
                .unwrap(),
            None
        );
    }
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn deactivation_and_password_resets_end_sessions() {
    let e = Env::new().await;
    let (user_id, tokens) = login(&e).await;
    let auth = &e.state.usecases.auth;

    // A password reset ends every session of the account.
    auth.forgot_password("nongdan@example.vn".into())
        .await
        .unwrap();
    let reset: String =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                format!("SELECT token FROM password_reset_tokens WHERE user_id = {user_id}"),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "token")
            .unwrap();
    auth.reset_password(reset, "matkhaumoi123".into())
        .await
        .unwrap();
    assert_eq!(auth.refresh_session(&tokens.refresh).await.unwrap(), None);

    // A deactivated account cannot renew either.
    let login = auth
        .login("nongdan".into(), "matkhaumoi123".into())
        .await
        .unwrap();
    let fresh = auth.start_session(login.user_id).await.unwrap();
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE users SET is_active = FALSE WHERE id = {user_id}"
        ))
        .await
        .unwrap();
    assert_eq!(auth.refresh_session(&fresh.refresh).await.unwrap(), None);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn expired_tokens_are_refused_and_purged() {
    let e = Env::new().await;
    let (_, tokens) = login(&e).await;
    e.t.db
        .execute_unprepared("UPDATE refresh_tokens SET expires_at = now() - interval '1 second'")
        .await
        .unwrap();
    let auth = &e.state.usecases.auth;
    assert_eq!(auth.refresh_session(&tokens.refresh).await.unwrap(), None);
    assert_eq!(auth.purge_expired_sessions().await.unwrap(), 1);
    e.cleanup().await;
}
