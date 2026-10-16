//! Account lockout after repeated wrong passwords. Database tests need
//! TEST_DATABASE_URL and `--include-ignored`.

use application::constants::LOGIN_FAIL_MAX_ATTEMPTS;
use domain::ErrorCode;
use integration_tests::{register_provider, Env};
use sea_orm::ConnectionTrait;

async fn try_login(e: &Env, password: &str) -> Result<(), ErrorCode> {
    e.state
        .usecases
        .auth
        .login("nongdan".into(), password.into())
        .await
        .map(|_| ())
        .map_err(|err| err.code())
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
