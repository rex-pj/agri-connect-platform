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
           WHERE cr.name = 'Lúa ST25' AND l.label = 'Cai Lậy, Đồng Tháp'",
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
        .register(integration_tests::signup(
            "nongdan01@example.vn",
            "mậtkhẩuư",
            "Vườn Thử",
            "0901234567",
        ))
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
        .register(integration_tests::signup("x", "short", " ", "123"))
        .await;
    // LoginResponse carries a session token, so it deliberately has no Debug.
    let Err(err) = result else {
        panic!("registration with bad fields succeeded")
    };
    assert_eq!(err.code(), ErrorCode::ValidationFailed);
    let fields: Vec<_> = err.fields().iter().map(|f| f.field).collect();
    assert_eq!(
        fields,
        ["email", "password", "business_name", "business_phone"]
    );

    // Each message carries its own limit, not one borrowed from another field.
    let body = web::error_response::error_body(&integration_tests::state(t.db.clone()), &err);
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
                  u AS (INSERT INTO users (email, password_hash, role_id)
                        SELECT 'nongdan@example.vn', 'x', id FROM r RETURNING id)
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
        // The weekly totals keep the rules of the rows they were counted from.
        (
            "INSERT INTO call_click_weeks
               (week_starts_on, business_id, service_type_id, supply_type_id, channel, taps)
               VALUES ('2026-01-05', 1, 1, 2, 'call', 3)",
            "chk_call_click_weeks_one_type",
        ),
        (
            "INSERT INTO call_click_weeks (week_starts_on, business_id, channel, taps)
               VALUES ('2026-01-05', 1, 'fax', 3)",
            "chk_call_click_weeks_channel",
        ),
        (
            "INSERT INTO call_click_weeks (week_starts_on, business_id, channel, taps)
               VALUES ('2026-01-05', 1, 'call', 0)",
            "chk_call_click_weeks_taps",
        ),
        (
            "INSERT INTO search_event_weeks
               (week_starts_on, service_type_id, supply_type_id, searches, unmet)
               VALUES ('2026-01-05', 1, 2, 3, 0)",
            "chk_search_event_weeks_one_type",
        ),
        (
            "INSERT INTO search_event_weeks (week_starts_on, searches, unmet)
               VALUES ('2026-01-05', 3, 4)",
            "chk_search_event_weeks_counts",
        ),
        // A business is in a ward there is.
        (
            "UPDATE businesses SET ward_code = '99999' WHERE id = 1",
            "businesses_ward_code_fkey",
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
        // A setting's key is dotted lower-case words, like the registry's.
        (
            "INSERT INTO platform_settings (key, value) VALUES ('Support Phone', '0912345678')",
            "chk_platform_settings_key",
        ),
        // A commodity's prices stay on the board 1 to 60 days.
        (
            "INSERT INTO sectors (code, name) VALUES ('crop', 'Trồng trọt');
             INSERT INTO commodities (name, sector, price_fresh_days) VALUES ('Lúa', 'crop', 0)",
            "chk_commodities_price_fresh_days",
        ),
        // An information page's address is a slug, its title says
        // something, who sees it and its footer group are of the lists,
        // and a version of its text is never blank.
        (
            "INSERT INTO info_pages (code, title) VALUES ('Dieu Khoan', 'Điều khoản')",
            "chk_info_pages_code",
        ),
        (
            "INSERT INTO info_pages (code, title) VALUES ('dieu-khoan', '  ')",
            "chk_info_pages_title",
        ),
        (
            "INSERT INTO info_pages (code, title, stage) VALUES ('dieu-khoan', 'Điều khoản', 'public')",
            "chk_info_pages_stage",
        ),
        (
            "INSERT INTO info_pages (code, title, footer_group) VALUES ('dieu-khoan', 'Điều khoản', 'side')",
            "chk_info_pages_footer_group",
        ),
        (
            "INSERT INTO info_pages (id, code, title) VALUES (1, 'dieu-khoan', 'Điều khoản');
             INSERT INTO info_page_revisions (page_id, body, effective_on) VALUES (1, ' ', '2026-11-27')",
            "chk_info_page_revisions_body",
        ),
    ] {
        refused(db, sql, constraint).await;
    }

    CatalogueRepository::new(Arc::new(t.db.clone()))
        .apply(&CATALOGUE)
        .await
        .unwrap();
    // An account is named by its email, folded so its UNIQUE ignores case.
    refused(
        db,
        "INSERT INTO users (email, password_hash, role_id)
           SELECT 'NongDan@example.vn', 'x', id FROM roles WHERE code = 'provider'",
        "chk_users_email_lower",
    )
    .await;
    // The address a link moves an account to is folded like the one it
    // becomes, and never blank.
    db.execute_unprepared(
        "INSERT INTO users (email, password_hash, role_id)
           SELECT 'nongdan@example.vn', 'x', id FROM roles WHERE code = 'provider'",
    )
    .await
    .unwrap();
    for (new_email, constraint) in [
        (
            "Moi@example.vn",
            "chk_email_verification_tokens_new_email_lower",
        ),
        (" ", "chk_email_verification_tokens_new_email_not_blank"),
    ] {
        refused(
            db,
            &format!(
                "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at, new_email)
                   SELECT id, repeat('a', 64), now(), '{new_email}' FROM users
                    WHERE email = 'nongdan@example.vn'"
            ),
            constraint,
        )
        .await;
    }

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

/// `url` signed in as `user` / `password` instead of its own account.
fn as_user(url: &str, user: &str, password: &str) -> String {
    let (scheme, rest) = url.split_once("://").unwrap();
    let host = rest.rsplit_once('@').map_or(rest, |(_, host)| host);
    format!("{scheme}://{user}:{password}@{host}")
}

/// `deploy/db-init/roles.psql` with its psql variables filled in, one
/// statement per item: what the database container runs on first start.
fn role_statements(vars: &[(&str, &str)]) -> Vec<String> {
    let mut sql: String = include_str!("../../../../../deploy/db-init/roles.psql")
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    for (name, value) in vars {
        sql = sql
            .replace(&format!(":\"{name}\""), &format!("\"{value}\""))
            .replace(&format!(":'{name}'"), &format!("'{value}'"));
    }
    assert!(
        !sql.contains(":\"") && !sql.contains(":'"),
        "a variable is left: {sql}"
    );
    sql.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL (a superuser, as CI's)"]
async fn the_app_role_cannot_change_the_schema() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    let id = format!(
        "{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let name = format!("{}roles_{id}", integration_tests::TEST_DB_PREFIX);
    let (owner, app) = (format!("agri_owner_{id}"), format!("agri_app_{id}"));
    let superuser = sea_orm::Database::connect(&url).await.unwrap();
    superuser
        .execute_unprepared(&format!("CREATE DATABASE {name}"))
        .await
        .unwrap();
    let in_db = sea_orm::Database::connect(integration_tests::database_url(&url, &name))
        .await
        .unwrap();
    for statement in role_statements(&[
        ("owner_role", &owner),
        ("owner_pw", "owner-pw"),
        ("app_role", &app),
        ("app_pw", "app-pw"),
        ("db_name", &name),
    ]) {
        in_db
            .execute_unprepared(&statement)
            .await
            .unwrap_or_else(|e| panic!("{statement}: {e}"));
    }
    in_db.close().await.unwrap();

    // The owner migrates, as the server does at startup.
    let db_url = integration_tests::database_url(&url, &name);
    let owner_db = sea_orm::Database::connect(as_user(&db_url, &owner, "owner-pw"))
        .await
        .unwrap();
    {
        use migration::MigratorTrait;
        migration::Migrator::up(&owner_db, None).await.unwrap();
    }
    owner_db.close().await.unwrap();

    // The app role does everything the server does: seeds, loads the demo
    // data (most tables, the shared type sequence), signs a provider up.
    let app_db = sea_orm::Database::connect(as_user(&db_url, &app, "app-pw"))
        .await
        .unwrap();
    let state = integration_tests::state(app_db.clone());
    state
        .usecases
        .setup
        .seed_catalogue(&CATALOGUE)
        .await
        .unwrap();
    state.usecases.setup.seed_admin_units().await.unwrap();
    DemoDataRepository::new(Arc::new(app_db.clone()))
        .load()
        .await
        .unwrap();
    integration_tests::register_provider(&state, "nongdan", "0901234567").await;

    // But it never changes the schema.
    for ddl in [
        "CREATE TABLE intruder (id int)",
        "DROP TABLE users",
        "ALTER TABLE users ADD COLUMN x int",
        "TRUNCATE users CASCADE",
    ] {
        assert!(app_db.execute_unprepared(ddl).await.is_err(), "{ddl}");
    }
    app_db.close().await.unwrap();

    superuser
        .execute_unprepared(&format!("DROP DATABASE {name} WITH (FORCE)"))
        .await
        .unwrap();
    superuser
        .execute_unprepared(&format!("DROP ROLE \"{app}\"; DROP ROLE \"{owner}\""))
        .await
        .unwrap();
}
