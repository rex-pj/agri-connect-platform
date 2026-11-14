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
async fn ready_is_200_with_a_database() {
    let t = TestDb::create().await;
    let router = integration_tests::router(integration_tests::state(t.db.clone()));
    let req = axum::http::Request::get("/health/ready")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = integration_tests::send(&router, req).await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);
    t.cleanup().await;
}

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
    // The catalogue, areas and crops are demo data: startup writes none.
    for table in [
        "commodities",
        "service_types",
        "supply_types",
        "locations",
        "crops",
    ] {
        let n = count(&t.db, &format!("SELECT COUNT(*) AS n FROM {table}")).await;
        assert_eq!(n, 0, "{table}");
    }
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
        default_roles: &["provider"],
    }];
    let later = Catalogue {
        roles: CATALOGUE.roles,
        permissions: EXTENDED,
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
async fn demo_data_loads_once_and_removal_keeps_what_real_data_uses() {
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
    let admin = integration_tests::first_admin(&state).await;
    let report = state.usecases.setup.load_demo(&admin).await.unwrap();
    assert_eq!(
        (
            report.commodities,
            report.categories,
            report.locations,
            report.crops
        ),
        (14, 12, 20, 19)
    );
    assert_eq!(
        (report.businesses, report.services, report.products),
        (10, 11, 14)
    );
    let counts = DemoDataRepository::new(db.clone()).count().await.unwrap();
    assert_eq!(
        (
            counts.commodities,
            counts.categories,
            counts.locations,
            counts.crops
        ),
        (14, 12, 20, 19)
    );
    assert_eq!(
        (counts.businesses, counts.services, counts.products),
        (10, 11, 14)
    );
    let status = state.usecases.setup.demo_status(&admin).await.unwrap();
    assert_eq!((status.demo.businesses, status.business_count), (10, 11));

    let again = state.usecases.setup.load_demo(&admin).await.unwrap_err();
    assert_eq!(again.code(), ErrorCode::DemoAlreadyLoaded);

    // Real data that uses the demo catalogue: a real shop's service of a demo
    // service type, a price of a demo crop in a demo area.
    t.db.execute_unprepared(
        "INSERT INTO services (business_id, service_type_id, name)
           SELECT b.id, c.id, 'Múc bùn' FROM businesses b, service_types c
           WHERE NOT b.is_demo AND c.slug = 'muc-bun';
         INSERT INTO crop_prices (crop_id, location_id, observed_on, price_low, price_high)
           SELECT cr.id, l.id, CURRENT_DATE, 7000, 7200 FROM crops cr, locations l
           WHERE cr.name = 'Lúa ST25' AND l.label = 'Đồng Tháp — Cai Lậy'",
    )
    .await
    .unwrap();

    assert_eq!(state.usecases.setup.remove_demo(&admin).await.unwrap(), 10);
    assert_eq!(
        count(&t.db, "SELECT COUNT(*) AS n FROM businesses").await,
        1
    );
    assert_eq!(count(&t.db, "SELECT COUNT(*) AS n FROM products").await, 0);
    // What real data uses stays, and is real from then on; the rest is gone:
    // the service's type, the price's area and crop, and that crop's
    // commodity.
    for (table, n) in [
        ("commodities", 1),
        ("service_types", 1),
        ("supply_types", 0),
        ("locations", 1),
        ("crops", 1),
    ] {
        let all = count(&t.db, &format!("SELECT COUNT(*) AS n FROM {table}")).await;
        assert_eq!(all, n, "{table}");
        let demo = count(
            &t.db,
            &format!("SELECT COUNT(*) AS n FROM {table} WHERE is_demo"),
        )
        .await;
        assert_eq!(demo, 0, "{table}");
    }
    assert_eq!(
        count(&t.db, "SELECT COUNT(*) AS n FROM crop_prices").await,
        1
    );

    let status = state.usecases.setup.demo_status(&admin).await.unwrap();
    assert!(!status.is_seeded);
    assert_eq!(status.platform.roles, CATALOGUE.roles.len() as u64);

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

/// Runs `sql`, which must be refused by the constraint `constraint`.
async fn refused(db: &DatabaseConnection, sql: &str, constraint: &str) {
    match db.execute_unprepared(sql).await {
        Ok(_) => panic!("{constraint} let this through: {sql}"),
        Err(e) => assert!(
            e.to_string().contains(constraint),
            "{sql}\nwas refused, but not by {constraint}: {e}"
        ),
    }
}

/// What the application checks before a write, the schema holds on its own:
/// a row cannot contradict the rows it points to.
#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_schema_refuses_rows_that_contradict_each_other() {
    let t = TestDb::create().await;
    let db = &t.db;
    db.execute_unprepared(
        "INSERT INTO businesses (id, name, phone)
           VALUES (1, 'Vườn A', '0901111111'), (2, 'Vườn B', '0902222222');
         INSERT INTO service_types (id, name, slug) VALUES (1, 'Múc bùn', 'muc-bun');
         INSERT INTO supply_types (id, name, slug) VALUES (2, 'Phân bón', 'phan-bon');
         INSERT INTO services (id, business_id, service_type_id, name)
           VALUES (1, 1, 1, 'Múc bùn'), (2, 2, 1, 'Múc bùn');
         INSERT INTO products (id, business_id, supply_type_id, name)
           VALUES (1, 1, 2, 'NPK'), (2, 2, 2, 'NPK');
         INSERT INTO inquiries (id, business_id, service_id, topic, requester_name, requester_phone)
           VALUES (1, 1, 1, 'Múc bùn', 'Chú Tư', '0987654321'),
                  (2, 2, 2, 'Múc bùn', 'Chú Tư', '0987654321');
         INSERT INTO provinces (code, name, full_name, unit_type, center) VALUES
           ('82', 'Đồng Tháp', 'Tỉnh Đồng Tháp', 'province', 'SRID=4326;POINT(106 10)'),
           ('91', 'An Giang', 'Tỉnh An Giang', 'province', 'SRID=4326;POINT(105 10)');
         INSERT INTO wards (code, province_code, name, full_name, unit_type, center, boundary)
           VALUES ('28439', '82', 'Bình Phú', 'Xã Bình Phú', 'commune', 'SRID=4326;POINT(106 10)',
             'SRID=4326;MULTIPOLYGON(((105.9 9.9, 106.1 9.9, 106.1 10.1, 105.9 10.1, 105.9 9.9)))');
         INSERT INTO partners (id, name, kind) VALUES (1, 'Đại lý', 'dealer');
         SELECT setval('inquiries_id_seq', 2);
         SELECT setval('catalogue_type_ids', 2)",
    )
    .await
    .unwrap();

    for (sql, constraint) in [
        // A contact request, a tap and a review name only their own
        // business's service, supply and request.
        (
            "INSERT INTO inquiries (business_id, service_id, topic, requester_name, requester_phone)
               VALUES (1, 2, 'Múc bùn', 'Chú Tư', '0987654321')",
            "fk_inquiries_service",
        ),
        (
            "INSERT INTO inquiries (business_id, product_id, topic, requester_name, requester_phone)
               VALUES (1, 2, 'NPK', 'Chú Tư', '0987654321')",
            "fk_inquiries_product",
        ),
        (
            "INSERT INTO inquiries
               (business_id, service_id, product_id, topic, requester_name, requester_phone)
               VALUES (1, 1, 1, 'NPK', 'Chú Tư', '0987654321')",
            "chk_inquiries_about_one",
        ),
        (
            "INSERT INTO call_clicks (business_id, service_id) VALUES (1, 2)",
            "fk_call_clicks_service",
        ),
        (
            "INSERT INTO call_clicks (business_id, product_id) VALUES (1, 2)",
            "fk_call_clicks_product",
        ),
        (
            "INSERT INTO call_clicks (business_id, service_id, product_id) VALUES (1, 1, 1)",
            "chk_call_clicks_about_one",
        ),
        (
            "INSERT INTO reviews (business_id, inquiry_id, rating, reviewer_name, reviewer_phone_hash)
               VALUES (1, 2, 5, 'Chú Tư', encode(sha256('0987654321'), 'hex'))",
            "fk_reviews_inquiry",
        ),
        (
            "INSERT INTO reviews (business_id, rating, reviewer_name, reviewer_phone_hash)
               VALUES (1, 5, 'Chú Tư', '0987654321')",
            "chk_reviews_phone_hash",
        ),
        // A confirmation link is stored as the hash of its token, never raw.
        (
            "WITH r AS (INSERT INTO roles (code, name) VALUES ('provider', 'provider')
                        RETURNING id),
                  u AS (INSERT INTO users (username, email, password_hash, role_id)
                        SELECT 'nongdan', 'nongdan@example.vn', 'x', id FROM r RETURNING id)
             INSERT INTO email_verification_tokens (user_id, token_hash, expires_at)
               SELECT id, 'lien-ket', now() FROM u",
            "chk_email_verification_tokens_hash",
        ),
        (
            "INSERT INTO reviews (business_id, rating, reviewer_name, reviewer_phone_hash, reply)
               VALUES (1, 5, 'Chú Tư', encode(sha256('0987654321'), 'hex'), 'Cảm ơn')",
            "chk_reviews_reply",
        ),
        // A contract's ward is a ward of its province.
        (
            "INSERT INTO partner_contracts
               (partner_id, province_code, ward_code, starts_on, ends_on, monthly_fee)
               VALUES (1, '91', '28439', CURRENT_DATE, CURRENT_DATE, 0)",
            "fk_partner_contracts_ward",
        ),
        // One id and one slug across the two type tables.
        (
            "INSERT INTO supply_types (name, slug) VALUES ('Múc bùn', 'muc-bun')",
            "uq_catalogue_types_distinct",
        ),
        (
            "INSERT INTO supply_types (id, name, slug) VALUES (1, 'Giống', 'giong')",
            "uq_catalogue_types_distinct",
        ),
        (
            "UPDATE service_types SET slug = 'phan-bon' WHERE id = 1",
            "uq_catalogue_types_distinct",
        ),
        (
            "INSERT INTO service_types (name, slug) VALUES ('Sửa máy', 'Sửa máy')",
            "chk_service_types_slug",
        ),
        (
            "INSERT INTO search_events (service_type_id, supply_type_id, results) VALUES (1, 2, 0)",
            "chk_search_events_one_type",
        ),
        // Text that must say something; a code in its one spelling.
        (
            "INSERT INTO businesses (name, phone) VALUES ('  ', '0903333333')",
            "chk_businesses_name_not_blank",
        ),
        (
            "INSERT INTO sectors (code, name) VALUES ('Trồng trọt', 'Trồng trọt')",
            "chk_sectors_code",
        ),
        (
            "INSERT INTO locations (label, lat, lng) VALUES ('Cai Lậy', 106.03, 10.41)",
            "chk_locations_point",
        ),
        (
            "INSERT INTO provinces (code, name, full_name, unit_type, center)
               VALUES ('p82', 'X', 'Tỉnh X', 'province', 'SRID=4326;POINT(106 10)')",
            "chk_provinces_code",
        ),
    ] {
        refused(db, sql, constraint).await;
    }

    CatalogueRepository::new(Arc::new(t.db.clone()))
        .apply(&CATALOGUE)
        .await
        .unwrap();
    refused(
        db,
        "INSERT INTO users (username, email, password_hash, role_id)
           SELECT 'NongDan', 'nongdan@example.vn', 'x', id FROM roles WHERE code = 'provider'",
        "chk_users_username_lower",
    )
    .await;

    // A new review waits for an admin; deleting a service keeps the request.
    db.execute_unprepared(
        "INSERT INTO reviews (business_id, inquiry_id, rating, reviewer_name, reviewer_phone_hash)
           VALUES (1, 1, 5, 'Chú Tư', encode(sha256('0987654321'), 'hex'));
         DELETE FROM services WHERE id = 1",
    )
    .await
    .unwrap();
    assert_eq!(
        count(db, "SELECT COUNT(*) AS n FROM reviews WHERE is_visible").await,
        0
    );
    assert_eq!(
        count(
            db,
            "SELECT COUNT(*) AS n FROM inquiries
               WHERE id = 1 AND business_id = 1 AND service_id IS NULL"
        )
        .await,
        1
    );
    // A business goes with everything sent to it.
    db.execute_unprepared("DELETE FROM businesses WHERE id = 1")
        .await
        .unwrap();
    for table in ["inquiries", "reviews"] {
        let left = count(
            db,
            &format!("SELECT COUNT(*) AS n FROM {table} WHERE business_id = 1"),
        )
        .await;
        assert_eq!(left, 0, "{table}");
    }

    t.cleanup().await;
}
