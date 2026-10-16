//! Two requests at the same moment where only one may win. Database tests
//! need TEST_DATABASE_URL and `--include-ignored`.

use domain::ErrorCode;
use integration_tests::{known_reset_token, register_provider, Env};

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn two_setup_forms_at_once_make_one_admin() {
    let e = Env::new().await;
    let auth = &e.state.usecases.auth;
    let (a, b) = tokio::join!(
        auth.create_first_admin(
            "quantri1".into(),
            "a@example.vn".into(),
            "matkhau123".into(),
            None
        ),
        auth.create_first_admin(
            "quantri2".into(),
            "b@example.vn".into(),
            "matkhau123".into(),
            None
        ),
    );
    let codes: Vec<_> = [&a, &b]
        .iter()
        .map(|r| r.as_ref().err().map(|e| e.code()))
        .collect();
    assert!(codes.contains(&None), "one setup wins: {codes:?}");
    assert!(
        codes.contains(&Some(ErrorCode::AdminAlreadyExists)),
        "the other is told an admin exists: {codes:?}"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_reset_link_sent_twice_at_once_resets_once() {
    let e = Env::new().await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let auth = &e.state.usecases.auth;
    auth.forgot_password("nongdan@example.vn".into())
        .await
        .unwrap();
    let token = known_reset_token(&e, "lien-ket").await;
    let (a, b) = tokio::join!(
        auth.reset_password(token.clone(), "matkhaumot1".into()),
        auth.reset_password(token.clone(), "matkhauhai2".into()),
    );
    assert_eq!(
        [a.is_ok(), b.is_ok()].iter().filter(|ok| **ok).count(),
        1,
        "exactly one reset goes through"
    );
    e.cleanup().await;
}
