//! Shared helpers for the integration tests in `tests/`.

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use application::ports::{EmailService, TokenService};
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, Response},
    Router,
};
use infrastructure::{email_service::LogEmailService, jwt_token_service::JwtTokenService};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use tower::ServiceExt;
use web::{
    app_state::AppState,
    routings::app_routing::{AppRouting, RouterConfig},
    startup::build_state,
};

pub const TEST_JWT_SECRET: &str = "integration-test-secret-at-least-32-bytes";

pub fn token_service() -> Arc<dyn TokenService> {
    Arc::new(JwtTokenService::new(TEST_JWT_SECRET).unwrap())
}

pub fn email_service() -> Arc<dyn EmailService> {
    Arc::new(LogEmailService::new("http://localhost:3000".into()))
}

/// State over `db`, wired exactly as production wires it.
pub fn state(db: DatabaseConnection) -> AppState {
    build_state(
        web::startup::Databases::single(db),
        token_service(),
        email_service(),
        web::views::SiteConfig::default(),
    )
    .unwrap()
}

pub fn router(state: AppState) -> Router {
    AppRouting::build(state, RouterConfig::default())
}

pub fn router_with(state: AppState, config: RouterConfig) -> Router {
    AppRouting::build(state, config)
}

/// A router whose database is disconnected: good for every path that must
/// answer before touching the database (routing, auth, body parsing).
pub fn router_without_db() -> Router {
    router(state(DatabaseConnection::default()))
}

/// Sends one request. Adds the peer address the rate limiter needs, which a
/// real server would supply through `into_make_service_with_connect_info`.
pub async fn send(router: &Router, mut request: Request<Body>) -> Response<Body> {
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    router.clone().oneshot(request).await.unwrap()
}

pub async fn json_body(response: Response<Body>) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!(
            "body is not JSON ({e}): {}",
            String::from_utf8_lossy(&bytes)
        )
    })
}

/// A throwaway database per test, created from `TEST_DATABASE_URL` (a URL to
/// any database on a server where the user may CREATE DATABASE).
pub struct TestDb {
    pub db: DatabaseConnection,
    admin: DatabaseConnection,
    name: String,
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

impl TestDb {
    pub async fn create() -> TestDb {
        let url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must point at a Postgres server with PostGIS");
        let name = format!(
            "agri_test_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );

        let admin = Database::connect(&url).await.unwrap();
        admin
            .execute_unprepared(&format!("DROP DATABASE IF EXISTS {name}"))
            .await
            .unwrap();
        admin
            .execute_unprepared(&format!("CREATE DATABASE {name}"))
            .await
            .unwrap();

        let (base, query) = match url.split_once('?') {
            Some((b, q)) => (b, format!("?{q}")),
            None => (url.as_str(), String::new()),
        };
        let (server, _) = base.rsplit_once('/').unwrap();
        let db = Database::connect(format!("{server}/{name}{query}"))
            .await
            .unwrap();
        Migrator::up(&db, None).await.unwrap();

        TestDb { db, admin, name }
    }

    pub async fn cleanup(self) {
        self.db.close().await.unwrap();
        self.admin
            .execute_unprepared(&format!("DROP DATABASE IF EXISTS {}", self.name))
            .await
            .unwrap();
    }
}
