//! Against a real Postgres + PostGIS. Run with
//! `TEST_DATABASE_URL=postgres://user:pass@host/postgres cargo test --workspace -- --include-ignored`.
//! Each test gets its own freshly migrated database.

use application::catalogue::CATALOGUE;
use domain::{
    models::catalogue::{Catalogue, PermissionSeed},
    repositories::{
        catalogue_repository_trait::CatalogueRepositoryTrait,
        demo_data_repository_trait::DemoDataRepositoryTrait,
        permission_repository_trait::PermissionRepositoryTrait,
    },
    ErrorCode,
};
use infrastructure::{
    catalogue_repository::CatalogueRepository, demo_data_repository::DemoDataRepository,
    repositories::permission_repository::PermissionRepository,
};
use integration_tests::TestDb;
use sea_orm::{ConnectionTrait, DatabaseConnection, FromQueryResult, Statement};
use std::sync::Arc;

#[derive(FromQueryResult)]
struct N {
    n: i64,
}

async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
    N::find_by_statement(Statement::from_string(db.get_database_backend(), sql))
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .n
}

const PROVIDER_HAS_DELETE: &str = "SELECT COUNT(*) AS n FROM role_permissions rp
    JOIN roles r ON r.id = rp.role_id JOIN permissions p ON p.id = rp.permission_id
    WHERE r.code = 'provider' AND p.code = 'own.listing.delete'";

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn fresh_database_gets_the_catalogue_once() {
    let t = TestDb::create().await;
    let repo = CatalogueRepository::new(Arc::new(t.db.clone()));

    let first = repo.apply(&CATALOGUE).await.unwrap();
    assert!(first.bootstrapped_grants);
    assert_eq!(first.roles_inserted, CATALOGUE.roles.len() as u64);
    assert_eq!(
        first.permissions_inserted,
        CATALOGUE.permissions.len() as u64
    );
    assert_eq!(first.categories_inserted, CATALOGUE.categories.len() as u64);
    assert_eq!(first.locations_inserted, CATALOGUE.locations.len() as u64);
    let grants: usize = CATALOGUE
        .permissions
        .iter()
        .map(|p| p.default_roles.len())
        .sum();
    assert_eq!(first.grants_inserted, grants as u64);

    let second = repo.apply(&CATALOGUE).await.unwrap();
    assert!(!second.bootstrapped_grants);
    assert_eq!(
        (
            second.roles_inserted,
            second.permissions_inserted,
            second.grants_inserted
        ),
        (0, 0, 0)
    );
    assert_eq!(
        (second.categories_inserted, second.locations_inserted),
        (0, 0)
    );
    assert!(second.unknown_permissions.is_empty());

    t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_revoked_grant_stays_revoked_across_restarts() {
    let t = TestDb::create().await;
    let repo = CatalogueRepository::new(Arc::new(t.db.clone()));
    repo.apply(&CATALOGUE).await.unwrap();

    t.db.execute_unprepared(
        "DELETE FROM role_permissions WHERE role_id = (SELECT id FROM roles WHERE code = 'provider')
         AND permission_id = (SELECT id FROM permissions WHERE code = 'own.listing.delete')",
    )
    .await
    .unwrap();

    repo.apply(&CATALOGUE).await.unwrap();
    assert_eq!(count(&t.db, PROVIDER_HAS_DELETE).await, 0);

    t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_permission_added_later_reaches_its_default_roles() {
    let t = TestDb::create().await;
    let repo = CatalogueRepository::new(Arc::new(t.db.clone()));
    repo.apply(&CATALOGUE).await.unwrap();

    static EXTENDED: &[PermissionSeed] = &[PermissionSeed {
        key: "own.inquiry.read",
        name: "Xem yêu cầu gửi tới mình",
        default_roles: &["provider"],
    }];
    let later = Catalogue {
        roles: CATALOGUE.roles,
        permissions: EXTENDED,
        categories: &[],
        locations: &[],
    };
    let report = repo.apply(&later).await.unwrap();
    assert_eq!(
        (report.permissions_inserted, report.grants_inserted),
        (1, 1)
    );
    // Everything the shorter catalogue no longer lists is reported, not deleted.
    assert_eq!(
        report.unknown_permissions.len(),
        CATALOGUE.permissions.len()
    );
    assert_eq!(count(&t.db, PROVIDER_HAS_DELETE).await, 1);

    t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn admin_edits_to_categories_survive_restarts() {
    let t = TestDb::create().await;
    let repo = CatalogueRepository::new(Arc::new(t.db.clone()));
    repo.apply(&CATALOGUE).await.unwrap();

    t.db.execute_unprepared("UPDATE categories SET slug = 'bun-ao' WHERE slug = 'muc-bun'")
        .await
        .unwrap();
    repo.apply(&CATALOGUE).await.unwrap();

    assert_eq!(
        count(
            &t.db,
            "SELECT COUNT(*) AS n FROM categories WHERE slug = 'muc-bun'"
        )
        .await,
        0
    );
    assert_eq!(
        count(&t.db, "SELECT COUNT(*) AS n FROM categories").await,
        CATALOGUE.categories.len() as i64
    );

    t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn demo_data_loads_once_and_removal_spares_real_listings() {
    let t = TestDb::create().await;
    let db = Arc::new(t.db.clone());
    CatalogueRepository::new(db.clone())
        .apply(&CATALOGUE)
        .await
        .unwrap();
    t.db.execute_unprepared(
        "INSERT INTO businesses (name, phone) VALUES ('Cơ sở thật', '0909999999')",
    )
    .await
    .unwrap();

    let state = integration_tests::state(t.db.clone());
    let report = state.usecases.setup.load_demo().await.unwrap();
    assert_eq!(
        (report.businesses, report.services, report.products),
        (6, 6, 10)
    );
    assert_eq!(
        DemoDataRepository::new(db.clone()).count().await.unwrap(),
        6
    );

    let again = state.usecases.setup.load_demo().await.unwrap_err();
    assert_eq!(again.code(), ErrorCode::DemoAlreadyLoaded);

    assert_eq!(state.usecases.setup.remove_demo().await.unwrap(), 6);
    assert_eq!(
        count(&t.db, "SELECT COUNT(*) AS n FROM businesses").await,
        1
    );
    assert_eq!(count(&t.db, "SELECT COUNT(*) AS n FROM products").await, 0);

    t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_registered_provider_holds_exactly_the_provider_permissions() {
    let t = TestDb::create().await;
    let db = Arc::new(t.db.clone());
    CatalogueRepository::new(db.clone())
        .apply(&CATALOGUE)
        .await
        .unwrap();

    let state = integration_tests::state(t.db.clone());
    let login = state
        .usecases
        .auth
        .register(
            "nongdan01".into(),
            "nongdan01@example.vn".into(),
            "mậtkhẩuư".into(),
            None,
            "Vườn Thử".into(),
            "0901234567".into(),
        )
        .await
        .unwrap();
    let claims = integration_tests::token_service()
        .verify(&login.token)
        .unwrap();

    let perms = PermissionRepository::new(db.clone());
    assert!(perms
        .has_permission(claims.user_id, "own.listing.create")
        .await
        .unwrap());
    assert!(!perms
        .has_permission(claims.user_id, "provider.verify")
        .await
        .unwrap());

    // Deactivating the role takes every grant away at once.
    t.db.execute_unprepared("UPDATE roles SET is_active = FALSE WHERE code = 'provider'")
        .await
        .unwrap();
    assert!(!perms
        .has_permission(claims.user_id, "own.listing.create")
        .await
        .unwrap());

    t.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn registration_reports_every_bad_field_at_once() {
    let t = TestDb::create().await;
    CatalogueRepository::new(Arc::new(t.db.clone()))
        .apply(&CATALOGUE)
        .await
        .unwrap();

    let result = integration_tests::state(t.db.clone())
        .usecases
        .auth
        .register(
            "ab".into(),
            "x@example.vn".into(),
            "short".into(),
            None,
            " ".into(),
            "123".into(),
        )
        .await;
    // LoginResponse carries a session token, so it deliberately has no Debug.
    let Err(err) = result else {
        panic!("registration with bad fields succeeded")
    };
    assert_eq!(err.code(), ErrorCode::ValidationFailed);
    let fields: Vec<_> = err.fields().iter().map(|f| f.field).collect();
    assert_eq!(
        fields,
        ["username", "password", "business_name", "business_phone"]
    );

    // Each message carries its own limit, not one borrowed from another field.
    let body = web::error_response::error_body(&integration_tests::state(t.db.clone()), &err);
    assert_eq!(
        body.field_errors["username"],
        ["Tên đăng nhập phải có ít nhất 3 ký tự."]
    );
    assert_eq!(
        body.field_errors["password"],
        ["Mật khẩu phải có ít nhất 8 ký tự."]
    );

    t.cleanup().await;
}
