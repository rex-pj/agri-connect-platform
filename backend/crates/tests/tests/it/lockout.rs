//! Account lockout after repeated wrong passwords, what a wrong password is
//! told, and the cap on reset links. Database tests need
//! TEST_DATABASE_URL and `--include-ignored`.

use application::constants::{LOGIN_FAIL_MAX_ATTEMPTS, PASSWORD_RESET_LINKS_MAX};
use domain::ErrorCode;
use integration_tests::{register_provider, Env};
use sea_orm::{ConnectionTrait, Statement};

async fn try_login_as(e: &Env, username: &str, password: &str) -> Result<(), ErrorCode> {
    e.state
        .usecases
        .auth
        .login(username.into(), password.into())
        .await
        .map(|_| ())
        .map_err(|err| err.code())
}

async fn try_login(e: &Env, password: &str) -> Result<(), ErrorCode> {
    try_login_as(e, "nongdan", password).await
}

async fn count(e: &Env, sql: &str) -> i64 {
    e.t.db
        .query_one(Statement::from_string(e.t.db.get_database_backend(), sql))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "n")
        .unwrap()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_wrong_password_learns_nothing_about_the_account() {
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let wrong = Err(ErrorCode::InvalidCredentials);

    // No such account: the same answer as a wrong password.
    assert_eq!(try_login_as(&e, "khongco", "matkhau123").await, wrong);

    // Deactivated: only the right password is told so.
    e.t.db
        .execute_unprepared("UPDATE users SET is_active = false")
        .await
        .unwrap();
    assert_eq!(try_login(&e, "sai").await, wrong);
    assert_eq!(
        try_login(&e, "matkhau123").await,
        Err(ErrorCode::AccountDeactivated)
    );
    assert_eq!(
        count(&e, "SELECT failed_login_attempts::bigint AS n FROM users").await,
        0,
        "a guess at a deactivated account is not a strike"
    );

    // Locked: likewise, and a guess does not push the lock further.
    e.t.db
        .execute_unprepared(
            "UPDATE users SET is_active = true, locked_until = now() + interval '5 minutes'",
        )
        .await
        .unwrap();
    assert_eq!(try_login(&e, "sai").await, wrong);
    assert_eq!(
        try_login(&e, "matkhau123").await,
        Err(ErrorCode::AccountLocked)
    );
    assert_eq!(
        count(
            &e,
            "SELECT count(*) AS n FROM users WHERE locked_until < now() + interval '6 minutes'"
        )
        .await,
        1,
        "the lock ends when it was going to"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn reset_links_of_one_account_are_capped() {
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    for _ in 0..PASSWORD_RESET_LINKS_MAX + 2 {
        e.state
            .usecases
            .auth
            .forgot_password("nongdan@example.vn".into())
            .await
            .unwrap();
    }
    assert_eq!(
        count(&e, "SELECT count(*) AS n FROM password_reset_tokens").await,
        PASSWORD_RESET_LINKS_MAX as i64
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_lock_that_ran_out_starts_a_fresh_count() {
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;

    for _ in 0..LOGIN_FAIL_MAX_ATTEMPTS {
        assert_eq!(
            try_login(&e, "sai").await,
            Err(ErrorCode::InvalidCredentials)
        );
    }
    // Locked: even the right password is refused for now.
    assert_eq!(
        try_login(&e, "matkhau123").await,
        Err(ErrorCode::AccountLocked)
    );

    // The lock runs out.
    e.t.db
        .execute_unprepared("UPDATE users SET locked_until = now() - interval '1 second'")
        .await
        .unwrap();
    // One typo after it is one strike, not a new lock.
    assert_eq!(
        try_login(&e, "sai").await,
        Err(ErrorCode::InvalidCredentials)
    );
    assert_eq!(try_login(&e, "matkhau123").await, Ok(()));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn sign_ins_and_password_changes_are_audited() {
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let auth = &e.state.usecases.auth;

    assert_eq!(
        try_login(&e, "sai").await,
        Err(ErrorCode::InvalidCredentials)
    );
    // A guess at a name nobody has is not about any account.
    assert_eq!(
        try_login_as(&e, "khongco", "sai").await,
        Err(ErrorCode::InvalidCredentials)
    );
    let login = auth
        .login("nongdan".into(), "matkhau123".into())
        .await
        .unwrap();
    auth.change_password(login.user_id, "matkhau123".into(), "matkhaumoi123".into())
        .await
        .unwrap();
    auth.forgot_password("nongdan@example.vn".into())
        .await
        .unwrap();
    let link = integration_tests::known_reset_token(&e, "lien-ket").await;
    // A made-up link is refused and leaves the real one usable.
    assert_eq!(
        auth.reset_password("khong-co".into(), "matkhaukhac123".into())
            .await
            .err()
            .map(|e| e.code()),
        Some(ErrorCode::ResetTokenInvalid)
    );
    auth.reset_password(link, "matkhaukhac123".into())
        .await
        .unwrap();

    let line: String =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                format!(
                    "SELECT string_agg(action, ',' ORDER BY id) AS line FROM audit_logs \
                     WHERE entity_type = 'user' AND entity_id = {}",
                    login.user_id
                ),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "line")
            .unwrap();
    assert_eq!(line, "login_failed,login,change_password,reset_password");
    e.cleanup().await;
}
