//! Shared helpers for the integration tests in `tests/`.

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use application::{
    catalogue::CATALOGUE,
    constants::{DEFAULT_UPLOAD_MAX_BYTES, REFRESH_TOKEN_DEFAULT_DAYS},
    ports::{EmailService, TokenService},
};
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{header, Method, Request, Response, StatusCode},
    Router,
};
use domain::repositories::catalogue_repository_trait::CatalogueRepositoryTrait;
use infrastructure::{
    catalogue_repository::CatalogueRepository, email_service::LogEmailService,
    file_storage::ObjectFileStorage, jwt_token_service::JwtTokenService,
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use tower::ServiceExt;
use web::{
    app_state::AppState,
    routings::app_routing::{AppRouting, RouterConfig},
    startup::{build_state, Uploads},
};

pub const TEST_JWT_SECRET: &str = "integration-test-secret-at-least-32-bytes";

pub fn token_service() -> Arc<dyn TokenService> {
    Arc::new(JwtTokenService::new(TEST_JWT_SECRET).unwrap())
}

pub fn email_service() -> Arc<dyn EmailService> {
    Arc::new(LogEmailService::new("http://localhost:3000".into()))
}

/// Uploads kept in memory, with the production size cap unless given.
pub fn memory_uploads(max_bytes: Option<usize>) -> Uploads {
    Uploads {
        storage: Arc::new(ObjectFileStorage::in_memory()),
        max_bytes: max_bytes.unwrap_or(DEFAULT_UPLOAD_MAX_BYTES),
    }
}

/// State over `db`, wired exactly as production wires it.
pub fn state(db: DatabaseConnection) -> AppState {
    state_with_uploads(db, memory_uploads(None))
}

pub fn state_with_uploads(db: DatabaseConnection, uploads: Uploads) -> AppState {
    build_state(
        web::startup::Databases::single(db),
        token_service(),
        email_service(),
        web::views::SiteConfig::default(),
        uploads,
        REFRESH_TOKEN_DEFAULT_DAYS,
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
            .execute_unprepared(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
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

    /// FORCE: a pooled connection can still be closing when the drop runs;
    /// without it the drop fails now and then with "being accessed by other users".
    pub async fn cleanup(self) {
        self.db.close().await.unwrap();
        self.admin
            .execute_unprepared(&format!(
                "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                self.name
            ))
            .await
            .unwrap();
    }
}

/// Creates the first admin on `state`'s database and returns their session
/// actor, with exactly the permissions the catalogue grants the admin role.
pub async fn first_admin(state: &AppState) -> application::authz::Actor {
    let login = state
        .usecases
        .auth
        .create_first_admin(
            "quantri".into(),
            "quantri@example.vn".into(),
            "matkhau123".into(),
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("create_first_admin: {e}"));
    let claims = token_service().verify(&login.token).unwrap();
    state
        .usecases
        .auth
        .actor(claims.user_id)
        .await
        .unwrap()
        .unwrap()
}

/// A migrated, catalogue-seeded database with the production state and a
/// router over it. Call `cleanup` at the end of the test.
pub struct Env {
    pub t: TestDb,
    pub state: AppState,
    pub router: Router,
}

impl Env {
    pub async fn new() -> Env {
        Self::with(None, RouterConfig::default()).await
    }

    /// `max_upload_bytes`: None keeps the production cap.
    pub async fn with(max_upload_bytes: Option<usize>, config: RouterConfig) -> Env {
        let t = TestDb::create().await;
        CatalogueRepository::new(Arc::new(t.db.clone()))
            .apply(&CATALOGUE)
            .await
            .unwrap();
        let state = state_with_uploads(t.db.clone(), memory_uploads(max_upload_bytes));
        let router = router_with(state.clone(), config);
        Env { t, state, router }
    }

    pub async fn cleanup(self) {
        self.t.cleanup().await;
    }
}

/// Registers a provider through the real flow (business "Vườn {username}")
/// and returns their session token.
pub async fn register_provider(state: &AppState, username: &str, phone: &str) -> String {
    state
        .usecases
        .auth
        .register(
            username.into(),
            format!("{username}@example.vn"),
            "matkhau123".into(),
            None,
            format!("Vườn {username}"),
            phone.into(),
        )
        .await
        .unwrap_or_else(|e| panic!("register: {e}"))
        .token
}

/// Creates a product through the provider API; `json` is the request body.
pub async fn create_product(env: &Env, token: &str, json: &str) -> i32 {
    let req = Request::post("/api/provider/products")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json.to_owned()))
        .unwrap();
    let res = send(&env.router, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    json_body(res).await["id"].as_i64().unwrap() as i32
}

pub const MULTIPART_BOUNDARY: &str = "agri-test-boundary";

/// A real JPEG with a gradient, so resizing has something to work on.
pub fn photo(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 120])
    });
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&img)
        .unwrap();
    out
}

/// A multipart body with one file part named `field`.
pub fn multipart(field: &str, bytes: &[u8]) -> Vec<u8> {
    let b = MULTIPART_BOUNDARY;
    let mut body = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"anh.jpg\"\r\n\
         Content-Type: image/jpeg\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{b}--\r\n").as_bytes());
    body
}

/// PUT /api/provider/products/{id}/image with `body` (see `multipart`).
pub fn upload_request(product_id: i32, token: Option<&str>, body: Vec<u8>) -> Request<Body> {
    let mut req = Request::builder()
        .method(Method::PUT)
        .uri(format!("/api/provider/products/{product_id}/image"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}"),
        );
    if let Some(token) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    req.body(Body::from(body)).unwrap()
}

/// Uploads a small photo for the product and returns the new file id.
pub async fn put_photo(env: &Env, product_id: i32, token: &str) -> String {
    let res = send(
        &env.router,
        upload_request(product_id, Some(token), multipart("image", &photo(64, 48))),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await["image_file_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// (id, slug) of the catalogue's categories of `kind` ("product", "service"),
/// in display order.
pub async fn categories_of(env: &Env, kind: &str) -> Vec<(i32, String)> {
    env.t
        .db
        .query_all(sea_orm::Statement::from_sql_and_values(
            env.t.db.get_database_backend(),
            "SELECT id, slug FROM categories WHERE type = $1 ORDER BY display_order",
            [kind.into()],
        ))
        .await
        .unwrap()
        .iter()
        .map(|r| {
            (
                r.try_get::<i32>("", "id").unwrap(),
                r.try_get::<String>("", "slug").unwrap(),
            )
        })
        .collect()
}
