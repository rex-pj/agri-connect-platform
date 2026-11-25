//! Account lockout after repeated wrong passwords, what a wrong password is
//! told, the cap on reset links and on sign-up tries. Database tests need
//! TEST_DATABASE_URL and `--include-ignored`.

use application::constants::{
    LOGIN_FAIL_ACCOUNT_MAX_ATTEMPTS, LOGIN_FAIL_MAX_ATTEMPTS, PASSWORD_RESET_LINKS_MAX,
};
use domain::ErrorCode;
use integration_tests::{email_of, register_provider, Env};
use sea_orm::{ConnectionTrait, Statement};

/// Signs in as `name` (`email_of`).
async fn try_login_as(e: &Env, name: &str, password: &str) -> Result<(), ErrorCode> {
    e.state
        .usecases
        .auth
        .login(email_of(name), password.into(), integration_tests::CLIENT)
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
async fn a_stranger_guessing_locks_out_only_themselves() {
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    const STRANGER: &str = "203.0.113.66";
    let from = |client: &'static str, password: &'static str| {
        let auth = e.state.usecases.auth.clone();
        async move {
            auth.login("nongdan@example.vn".into(), password.into(), client)
                .await
                .map(|_| ())
                .map_err(|err| err.code())
        }
    };

    for _ in 0..LOGIN_FAIL_MAX_ATTEMPTS {
        assert_eq!(
            from(STRANGER, "sai").await,
            Err(ErrorCode::InvalidCredentials)
        );
    }
    // That client is kept out, even with the right password…
    assert_eq!(
        from(STRANGER, "matkhau123").await,
        Err(ErrorCode::AccountLocked)
    );
    // …and its further guesses are not counted.
    assert_eq!(
        from(STRANGER, "sai").await,
        Err(ErrorCode::InvalidCredentials)
    );
    assert_eq!(
        count(&e, "SELECT failed_login_attempts::bigint AS n FROM users").await,
        i64::from(LOGIN_FAIL_MAX_ATTEMPTS)
    );
    // The owner, on their own phone, still gets in; that clears the count.
    assert_eq!(try_login(&e, "matkhau123").await, Ok(()));
    assert_eq!(
        count(&e, "SELECT failed_login_attempts::bigint AS n FROM users").await,
        0
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn guesses_from_many_clients_lock_the_account_itself() {
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let auth = &e.state.usecases.auth;
    // Each client stays under its own limit; together they reach the
    // account's.
    let per_client = LOGIN_FAIL_MAX_ATTEMPTS - 1;
    let clients = LOGIN_FAIL_ACCOUNT_MAX_ATTEMPTS as u32 / per_client + 1;
    for c in 0..clients {
        let client = format!("198.51.100.{c}");
        for _ in 0..per_client {
            let _ = auth.login("nongdan@example.vn".into(), "sai".into(), &client).await;
        }
    }
    assert_eq!(
        try_login(&e, "matkhau123").await,
        Err(ErrorCode::AccountLocked),
        "the right password waits for the lock to run out"
    );

    // The lock runs out: the right password gets in.
    e.t.db
        .execute_unprepared("UPDATE users SET locked_until = now() - interval '1 second'")
        .await
        .unwrap();
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
        .login(
            "nongdan@example.vn".into(),
            "matkhau123".into(),
            integration_tests::CLIENT,
        )
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

/// One client tries at most `SIGNUP_ATTEMPTS_PER_CLIENT` valid sign-ups a
/// day: each tells whether an email is taken, so nobody can
/// sweep a list of addresses through the form. A form refused for its own
/// mistakes tells nothing and is not counted; another client is not held.
#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn one_client_cannot_probe_emails_through_sign_up() {
    use application::constants::SIGNUP_ATTEMPTS_PER_CLIENT;
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let auth = &e.state.usecases.auth;
    let try_from = |client: &str, email: &str, password: &str| {
        let mut input = integration_tests::signup(email, password, "Vườn", "0907654321");
        input.client = client.to_owned();
        async move { auth.register(input).await.map(|_| ()).map_err(|e| e.code()) }
    };

    let probe = "203.0.113.7";
    assert_eq!(
        try_from(probe, "a@example.vn", "short").await,
        Err(ErrorCode::ValidationFailed),
        "a broken form is not a probe"
    );
    for _ in 0..SIGNUP_ATTEMPTS_PER_CLIENT {
        assert_eq!(
            try_from(probe, "nongdan@example.vn", "matkhau123").await,
            Err(ErrorCode::EmailTaken)
        );
    }
    assert_eq!(
        try_from(probe, "khachmoi@example.vn", "matkhau123").await,
        Err(ErrorCode::SignupDailyLimit),
        "even a sign-up that would succeed waits for tomorrow"
    );
    assert_eq!(
        try_from("198.51.100.9", "khachmoi@example.vn", "matkhau123").await,
        Ok(())
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_email_names_the_account_and_nothing_else_does() {
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let auth = &e.state.usecases.auth;

    // Folded as stored; the session answers with the stored address.
    let session = auth
        .login(
            "  NongDan@Example.VN ".into(),
            "matkhau123".into(),
            integration_tests::CLIENT,
        )
        .await
        .unwrap();
    assert_eq!(session.email, "nongdan@example.vn");
    assert_eq!(
        try_login_as(&e, "khongco@example.vn", "matkhau123").await,
        Err(ErrorCode::InvalidCredentials),
        "an unknown email answers as a wrong password"
    );

    // What is not an address names no account: refused before anything is
    // looked up, and never counted against the one it might have meant.
    for _ in 0..LOGIN_FAIL_MAX_ATTEMPTS {
        let err = auth
            .login("nongdan".into(), "sai".into(), integration_tests::CLIENT)
            .await
            .err()
            .expect("refused");
        assert!(
            err.fields()
                .iter()
                .any(|f| f.field == "email" && f.code == ErrorCode::InvalidEmail),
            "{err:?}"
        );
    }
    assert_eq!(try_login(&e, "matkhau123").await, Ok(()));

    // A deactivated account is told nothing on a wrong password.
    e.t.db
        .execute_unprepared("UPDATE users SET is_active = false")
        .await
        .unwrap();
    assert_eq!(
        try_login(&e, "sai").await,
        Err(ErrorCode::InvalidCredentials)
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_session_cannot_guess_its_own_current_password() {
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let auth = &e.state.usecases.auth;
    let user_id = auth
        .login(
            "nongdan@example.vn".into(),
            "matkhau123".into(),
            integration_tests::CLIENT,
        )
        .await
        .unwrap()
        .user_id;
    let change =
        |current: &str| auth.change_password(user_id, current.into(), "matkhaumoi123".into());

    for _ in 0..LOGIN_FAIL_MAX_ATTEMPTS {
        assert_eq!(
            change("sai").await.err().map(|e| e.code()),
            Some(ErrorCode::CurrentPasswordIncorrect)
        );
    }
    // Now even the right one is not checked: no more guesses.
    assert_eq!(
        change("matkhau123").await.err().map(|e| e.code()),
        Some(ErrorCode::TooManyRequests)
    );
    // Signing in still works: only the change is held back.
    assert_eq!(try_login(&e, "matkhau123").await, Ok(()));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_right_current_password_forgets_the_wrong_ones() {
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let auth = &e.state.usecases.auth;
    let user_id = auth
        .login(
            "nongdan@example.vn".into(),
            "matkhau123".into(),
            integration_tests::CLIENT,
        )
        .await
        .unwrap()
        .user_id;
    for _ in 1..LOGIN_FAIL_MAX_ATTEMPTS {
        assert!(auth
            .change_password(user_id, "sai".into(), "matkhaumoi123".into())
            .await
            .is_err());
    }
    auth.change_password(user_id, "matkhau123".into(), "matkhaumoi123".into())
        .await
        .unwrap();
    // Counted from zero again.
    assert_eq!(
        auth.change_password(user_id, "sai".into(), "matkhaukhac123".into())
            .await
            .err()
            .map(|e| e.code()),
        Some(ErrorCode::CurrentPasswordIncorrect)
    );
    e.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs TEST_DATABASE_URL"]
async fn sign_up_probes_sent_at_once_never_pass_the_cap() {
    use application::constants::SIGNUP_ATTEMPTS_PER_CLIENT;
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let mut runs = tokio::task::JoinSet::new();
    for _ in 0..SIGNUP_ATTEMPTS_PER_CLIENT * 3 {
        let state = e.state.clone();
        runs.spawn(async move {
            let mut input = integration_tests::signup(
                "nongdan@example.vn",
                "matkhau123",
                "Vườn",
                "0907654321",
            );
            input.client = "203.0.113.7".to_owned();
            state
                .usecases
                .auth
                .register(input)
                .await
                .map(|_| ())
                .map_err(|e| e.code())
        });
    }
    let mut probes = 0;
    while let Some(answer) = runs.join_next().await {
        match answer.unwrap() {
            Err(ErrorCode::EmailTaken) => probes += 1,
            Err(ErrorCode::SignupDailyLimit) => {}
            other => panic!("unexpected answer {other:?}"),
        }
    }
    assert_eq!(
        probes, SIGNUP_ATTEMPTS_PER_CLIENT,
        "only the day's tries learn anything"
    );
    e.cleanup().await;
}
