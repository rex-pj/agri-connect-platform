//! Browser sessions: refresh-token rotation and reuse detection, renewal by
//! the middleware, logout. Database tests need TEST_DATABASE_URL and
//! `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, Response, StatusCode},
};
use integration_tests::{known_reset_token, register_provider, send, Env};
use sea_orm::{ConnectionTrait, Statement};

use application::usecases::auth_usecase::{AuthUseCase, Renewal, SessionTokens};

/// The new pair of a renewal, `None` when there is none for any reason
/// (`renew_session`, the call the session middleware makes, says which).
trait RefreshSession {
    async fn refresh_session(&self, raw: &str) -> domain::Result<Option<SessionTokens>>;
}

impl RefreshSession for AuthUseCase {
    async fn refresh_session(&self, raw: &str) -> domain::Result<Option<SessionTokens>> {
        Ok(match self.renew_session(raw).await? {
            Renewal::Renewed(tokens) => Some(tokens),
            Renewal::Raced | Renewal::Ended => None,
        })
    }
}

async fn login(e: &Env) -> (i32, SessionTokens) {
    register_provider(&e.state, "nongdan", "0901234567").await;
    let login = e
        .state
        .usecases
        .auth
        .login(
            "nongdan".into(),
            "matkhau123".into(),
            integration_tests::CLIENT,
        )
        .await
        .unwrap();
    (login.user_id, login.session)
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
async fn a_renewal_survives_an_error_page() {
    let e = Env::new().await;
    let (_, tokens) = login(&e).await;

    // The first page after the access token expired is a dead link.
    let req = Request::get("/supplies/999999/khong-con")
        .header(header::ACCEPT, "text/html")
        .header(header::COOKIE, format!("refresh_token={}", tokens.refresh))
        .body(Body::empty())
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let set = set_cookies(&res);
    let refresh = cookie_value(&set, "refresh_token").expect("the rotated refresh cookie");
    assert!(cookie_value(&set, "auth_token").is_some());
    // The browser now holds a token that still renews.
    assert!(e
        .state
        .usecases
        .auth
        .refresh_session(&refresh)
        .await
        .unwrap()
        .is_some());
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_tab_that_loses_a_renewal_race_leaves_the_cookies_alone() {
    let e = Env::new().await;
    let (_, tokens) = login(&e).await;
    let page = || {
        Request::get("/")
            .header(header::ACCEPT, "text/html")
            .header(header::COOKIE, format!("refresh_token={}", tokens.refresh))
            .body(Body::empty())
            .unwrap()
    };
    let first = send(&e.router, page()).await;
    let winner = set_cookies(&first);
    assert!(cookie_value(&winner, "refresh_token").is_some());

    // A second tab sent the same old token a moment later.
    let second = send(&e.router, page()).await;
    assert_eq!(second.status(), StatusCode::OK);
    assert!(
        set_cookies(&second).is_empty(),
        "clearing here would wipe the winner's cookies"
    );
    // And the family is intact.
    let rotated = cookie_value(&winner, "refresh_token").unwrap();
    assert!(e
        .state
        .usecases
        .auth
        .refresh_session(&rotated)
        .await
        .unwrap()
        .is_some());
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
                .login(
                    "nongdan".into(),
                    "matkhau123".into(),
                    integration_tests::CLIENT,
                )
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
        // The access token goes with its session, not an hour later.
        let req = Request::get("/api/users/me")
            .header(header::COOKIE, format!("auth_token={}", tokens.access))
            .body(Body::empty())
            .unwrap();
        let res = send(&e.router, req).await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{path}");
    }
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_account_turned_off_and_on_again_logs_in_again() {
    let e = Env::new().await;
    let (user_id, tokens) = login(&e).await;
    let admin = integration_tests::first_admin(&e.state).await;
    let auth = &e.state.usecases.auth;
    for is_active in [false, true] {
        e.state
            .usecases
            .admin
            .toggle_user_active(&admin, user_id, is_active, None)
            .await
            .unwrap();
    }
    assert_eq!(auth.refresh_session(&tokens.refresh).await.unwrap(), None);
    let claims = integration_tests::token_service()
        .verify(&tokens.access)
        .unwrap();
    assert_eq!(
        auth.session_actor(&claims).await.err().map(|e| e.code()),
        Some(domain::ErrorCode::InvalidToken)
    );
    // The password still works: a new session is a new log-in.
    assert!(auth
        .login(
            "nongdan".into(),
            "matkhau123".into(),
            integration_tests::CLIENT
        )
        .await
        .is_ok());
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn deactivation_and_password_resets_end_sessions() {
    let e = Env::new().await;
    let (user_id, tokens) = login(&e).await;
    let auth = &e.state.usecases.auth;

    // A password reset ends every session of the account. Two links were
    // requested; the newer one is used.
    auth.forgot_password("nongdan@example.vn".into())
        .await
        .unwrap();
    let older = known_reset_token(&e, "lien-ket-cu").await;
    auth.forgot_password("nongdan@example.vn".into())
        .await
        .unwrap();
    let newer = known_reset_token(&e, "lien-ket-moi").await;
    auth.reset_password(newer, "matkhaumoi123".into())
        .await
        .unwrap();
    assert_eq!(auth.refresh_session(&tokens.refresh).await.unwrap(), None);
    assert_eq!(
        auth.reset_password(older, "matkhaukhac123".into())
            .await
            .err()
            .map(|e| e.code()),
        Some(domain::ErrorCode::ResetTokenInvalid),
        "an older link dies with the reset"
    );

    // A deactivated account cannot renew either.
    let login = auth
        .login(
            "nongdan".into(),
            "matkhaumoi123".into(),
            integration_tests::CLIENT,
        )
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
async fn a_password_change_signs_other_devices_out_at_once() {
    let e = Env::new().await;
    let (user_id, other_device) = login(&e).await;
    let me = |access: &str| {
        Request::get("/api/users/me")
            .header(header::AUTHORIZATION, format!("Bearer {access}"))
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        send(&e.router, me(&other_device.access)).await.status(),
        StatusCode::OK
    );

    // The cut-off is in whole seconds, like `iat`: let the old token's second pass.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let auth = &e.state.usecases.auth;
    auth.change_password(user_id, "matkhau123".into(), "matkhaumoi123".into())
        .await
        .unwrap();
    let this_device = auth.start_session(user_id).await.unwrap();

    let res = send(&e.router, me(&other_device.access)).await;
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "still within its hour"
    );
    assert_eq!(
        integration_tests::json_body(res).await["code"],
        "invalid_token"
    );
    assert_eq!(
        send(&e.router, me(&this_device.access)).await.status(),
        StatusCode::OK
    );
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
    // The sign-up's session and the login's.
    assert_eq!(auth.purge_expired_sessions().await.unwrap(), 2);
    e.cleanup().await;
}

/// The JSON login's token is in a session too: `DELETE /api/auth/tokens`
/// ends it on the next request, not when its hour runs out.
#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_json_api_logs_out_at_once() {
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let req = Request::post("/api/auth/tokens")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"username":"nongdan","password":"matkhau123"}"#,
        ))
        .unwrap();
    let res = send(&e.router, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = integration_tests::json_body(res).await;
    let token = body["token"].as_str().unwrap().to_owned();
    assert!(
        body.get("session").is_none(),
        "the refresh token stays inside"
    );

    let bearer = |method: &str, path: &str| {
        Request::builder()
            .method(method)
            .uri(path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        send(&e.router, bearer("GET", "/api/users/me"))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(&e.router, bearer("DELETE", "/api/auth/tokens"))
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    let res = send(&e.router, bearer("GET", "/api/users/me")).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        integration_tests::json_body(res).await["code"],
        "invalid_token"
    );
    // Ending it again changes nothing, and says so the same way.
    assert_eq!(
        send(&e.router, bearer("DELETE", "/api/auth/tokens"))
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    e.cleanup().await;
}
