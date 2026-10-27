//! Shared helpers for the integration tests in `tests/`.

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use application::{
    catalogue::CATALOGUE,
    constants::{DEFAULT_UPLOAD_MAX_BYTES, REFRESH_TOKEN_DEFAULT_DAYS},
    ports::{EmailService, InquiryNotice, TokenService},
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
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
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
    Arc::new(LogEmailService::new("http://localhost:5173".into()))
}

/// An `EmailService` that keeps the "new inquiry" emails it was asked to send,
/// so a test can read them. Reset emails are only logged, as in development.
#[derive(Default)]
pub struct RecordingEmail {
    inquiries: std::sync::Mutex<Vec<(String, InquiryNotice)>>,
}

impl RecordingEmail {
    /// (recipient, notice) of every "new inquiry" email so far. They are sent
    /// in the background, so this waits (up to 2 s) until `at_least` arrived.
    pub async fn inquiries(&self, at_least: usize) -> Vec<(String, InquiryNotice)> {
        for _ in 0..100 {
            let sent = self.inquiries.lock().unwrap().clone();
            if sent.len() >= at_least {
                return sent;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        self.inquiries.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl EmailService for RecordingEmail {
    async fn send_password_reset(&self, _to_email: &str, _token: &str) {}

    async fn send_new_inquiry(&self, to_email: &str, notice: &InquiryNotice) {
        self.inquiries
            .lock()
            .unwrap()
            .push((to_email.to_owned(), notice.clone()));
    }
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
    state_with_email(db, uploads, email_service())
}

pub fn state_with_email(
    db: DatabaseConnection,
    uploads: Uploads,
    email: Arc<dyn EmailService>,
) -> AppState {
    build_state(
        web::startup::Databases::single(db),
        token_service(),
        email,
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
    guard: DropGuard,
}

static COUNTER: AtomicU32 = AtomicU32::new(0);
static SWEPT: AtomicBool = AtomicBool::new(false);

const TEST_DB_PREFIX: &str = "agri_test_";
/// A test database older than this belongs to a run that was killed (Ctrl+C)
/// before it could drop it; no test runs this long.
const STALE_TEST_DB_SECS: u64 = 60 * 60;

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Drops the database when a test panics before `cleanup`. Drop cannot await,
/// so the drop runs on its own thread with its own runtime and connection.
struct DropGuard {
    url: String,
    name: String,
    armed: bool,
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let (url, name) = (self.url.clone(), self.name.clone());
        let dropped = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let admin = Database::connect(&url).await?;
                    admin.execute_unprepared(&drop_sql(&name)).await?;
                    admin.close().await
                })
        })
        .join();
        if !matches!(dropped, Ok(Ok(()))) {
            eprintln!("could not drop test database {}", self.name);
        }
    }
}

/// FORCE: a pooled connection can still be closing when the drop runs;
/// without it the drop fails now and then with "being accessed by other users".
fn drop_sql(name: &str) -> String {
    format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)")
}

/// Once per test process: drops test databases left by killed runs. Names
/// carry their creation time (`agri_test_{secs}_{pid}_{n}`), so a database a
/// concurrent run is still using is never touched. Names from before the time
/// was added (`agri_test_{pid}_{n}`) read as ancient and are dropped too.
async fn sweep_stale(admin: &DatabaseConnection) {
    if SWEPT.swap(true, Ordering::SeqCst) {
        return;
    }
    let rows = admin
        .query_all(Statement::from_string(
            admin.get_database_backend(),
            format!("SELECT datname FROM pg_database WHERE datname LIKE '{TEST_DB_PREFIX}%'"),
        ))
        .await
        .unwrap();
    let cutoff = unix_now().saturating_sub(STALE_TEST_DB_SECS);
    for row in rows {
        let name: String = row.try_get("", "datname").unwrap();
        let created = name
            .strip_prefix(TEST_DB_PREFIX)
            .and_then(|rest| rest.split('_').next())
            .and_then(|secs| secs.parse::<u64>().ok());
        if created.is_some_and(|secs| secs < cutoff) {
            admin.execute_unprepared(&drop_sql(&name)).await.unwrap();
        }
    }
}

impl TestDb {
    pub async fn create() -> TestDb {
        let url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must point at a Postgres server with PostGIS");
        let name = format!(
            "{TEST_DB_PREFIX}{}_{}_{}",
            unix_now(),
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );

        let admin = Database::connect(&url).await.unwrap();
        sweep_stale(&admin).await;
        // Armed before CREATE: a panic from here on still drops the database.
        let guard = DropGuard {
            url: url.clone(),
            name: name.clone(),
            armed: true,
        };
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

        TestDb {
            db,
            admin,
            name,
            guard,
        }
    }

    pub async fn cleanup(self) {
        let TestDb {
            db,
            admin,
            name,
            mut guard,
        } = self;
        db.close().await.unwrap();
        admin.execute_unprepared(&drop_sql(&name)).await.unwrap();
        guard.armed = false;
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
    /// What the app emailed (only the "new inquiry" emails are kept).
    pub mail: Arc<RecordingEmail>,
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
        let mail = Arc::new(RecordingEmail::default());
        let state = state_with_email(t.db.clone(), memory_uploads(max_upload_bytes), mail.clone());
        let router = router_with(state.clone(), config);
        Env {
            t,
            state,
            router,
            mail,
        }
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

/// Only the hash of a reset token is stored, and the email that carries the
/// raw one goes to the log. Re-keys the newest reset token to `raw`, so a
/// test can follow "the link in the latest email"; returns `raw`.
pub async fn known_reset_token(env: &Env, raw: &str) -> String {
    env.t
        .db
        .execute(sea_orm::Statement::from_sql_and_values(
            env.t.db.get_database_backend(),
            "UPDATE password_reset_tokens \
             SET token = encode(sha256(convert_to($1, 'UTF8')), 'hex') \
             WHERE id = (SELECT max(id) FROM password_reset_tokens)",
            [raw.into()],
        ))
        .await
        .unwrap();
    raw.to_owned()
}

/// The origin the test routers accept for non-GET requests.
pub const TEST_ORIGIN: &str = "http://localhost:5173";

/// A router that accepts `TEST_ORIGIN`, like production accepts APP_URL.
pub async fn env_with_origin() -> Env {
    Env::with(
        None,
        RouterConfig {
            allowed_origins: vec![TEST_ORIGIN.to_owned()],
            ..RouterConfig::default()
        },
    )
    .await
}

/// Encodes `pairs` the way a browser submits a form.
pub fn form_body(pairs: &[(&str, &str)]) -> String {
    let enc = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' => {
                    (b as char).to_string()
                }
                b' ' => "+".to_owned(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// A page form POST from our own origin, with an optional Cookie header.
pub fn post_form(path: &str, pairs: &[(&str, &str)], cookie: Option<&str>) -> Request<Body> {
    let mut req = Request::post(path)
        .header(header::ACCEPT, "text/html")
        .header(header::ORIGIN, TEST_ORIGIN)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    req.body(Body::from(form_body(pairs))).unwrap()
}

/// A browser page GET with an optional Cookie header.
pub fn get_page(path: &str, cookie: Option<&str>) -> Request<Body> {
    let mut req = Request::get(path).header(header::ACCEPT, "text/html");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    req.body(Body::empty()).unwrap()
}

pub async fn body_text(res: Response<Body>) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// `name=value; …` for the cookies a response sets (not the expiring ones),
/// to send back as a browser would.
pub fn cookies_from(res: &Response<Body>) -> String {
    res.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter(|c| !c.contains("Max-Age=0"))
        .filter_map(|c| c.split(';').next())
        .collect::<Vec<_>>()
        .join("; ")
}

/// The first admin ("quantri" / "matkhau123") created through the setup
/// page; returns that browser's session cookies.
pub async fn install_admin(env: &Env) -> String {
    let res = send(
        &env.router,
        post_form(
            "/setup",
            &[
                ("email", "admin@example.vn"),
                ("username", "quantri"),
                ("password", "matkhau123"),
                ("password2", "matkhau123"),
            ],
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "setup");
    cookies_from(&res)
}

/// Signs in through the login page; returns the session cookies.
pub async fn sign_in(env: &Env, username: &str, password: &str) -> String {
    let res = send(
        &env.router,
        post_form(
            "/login",
            &[("username", username), ("password", password)],
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "login as {username}");
    cookies_from(&res)
}

/// A page form sent as `multipart/form-data` from our own origin: text
/// `fields`, and `image` as the file part (an empty part when `None`, as a
/// browser sends for a file box left empty).
pub fn post_multipart_form(
    path: &str,
    fields: &[(&str, &str)],
    image: Option<&[u8]>,
    cookie: &str,
) -> Request<Body> {
    let b = MULTIPART_BOUNDARY;
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--{b}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    let (filename, bytes) = match image {
        Some(bytes) => ("anh.jpg", bytes),
        None => ("", &[][..]),
    };
    body.extend_from_slice(
        format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"{filename}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{b}--\r\n").as_bytes());
    Request::post(path)
        .header(header::ACCEPT, "text/html")
        .header(header::ORIGIN, TEST_ORIGIN)
        .header(header::COOKIE, cookie)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={b}"),
        )
        .body(Body::from(body))
        .unwrap()
}
