//! Configuration, dependency wiring and server startup. The only place that
//! reads environment variables or names concrete adapters.

use std::{net::SocketAddr, sync::Arc};

use application::{
    catalogue::CATALOGUE,
    constants::{
        DEFAULT_UPLOAD_MAX_BYTES, IMAGE_JPEG_QUALITY, IMAGE_MAIN_MAX_SIDE, IMAGE_MAX_PIXELS,
        IMAGE_THUMB_MAX_SIDE, REFRESH_TOKEN_DEFAULT_DAYS,
    },
    events::{
        AuditLogSubscriber, FileCleanupSubscriber, InProcessEventBus, InquiryNotificationSubscriber,
    },
    ports::{EmailService, EventBus, FileStorage, PasswordHasher, TokenService},
    usecases::{
        admin_usecase::{AdminDeps, AdminUseCase},
        auth_usecase::{AuthDeps, AuthUseCase},
        explore_usecase::{ExploreDeps, ExploreUseCase},
        health_usecase::HealthUseCase,
        media_usecase::{MediaDeps, MediaUseCase},
        permission_usecase::{PermissionDeps, PermissionUseCase},
        product_usecase::ProductUseCase,
        provider_dashboard_usecase::{ProviderDashboardDeps, ProviderDashboardUseCase},
        review_usecase::{ReviewDeps, ReviewUseCase},
        setup_usecase::SetupUseCase,
        stats_usecase::{StatsDeps, StatsUseCase},
    },
};
use infrastructure::{
    bcrypt_password_hasher::BcryptPasswordHasher,
    catalogue_repository::CatalogueRepository,
    demo_data_repository::DemoDataRepository,
    email_service::{EmailTexts, LogEmailService, SmtpConfig, SmtpEmailService},
    file_storage::ObjectFileStorage,
    image_processor::{ImageRules, RasterImageProcessor},
    jwt_token_service::JwtTokenService,
    repositories::{
        audit_log_repository::AuditLogRepository, business_repository::BusinessRepository,
        call_repository::CallRepository, category_repository::CategoryRepository,
        file_repository::FileRepository, health_repository::HealthRepository,
        inquiry_repository::InquiryRepository, location_repository::LocationRepository,
        permission_repository::PermissionRepository, product_repository::ProductRepository,
        refresh_token_repository::RefreshTokenRepository, review_repository::ReviewRepository,
        role_repository::RoleRepository, service_repository::ServiceRepository,
        user_repository::UserRepository,
    },
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use tracing::{info, warn};

use crate::{
    app_state::{AppState, UseCases},
    constants::{DB_MAX_CONNECTIONS, DEFAULT_APP_URL, DEFAULT_BIND_ADDR, DEFAULT_UPLOAD_DIR},
    locale::{Lang, Locale, Locales},
    middleware::{origin::origin_of, rate_limit::Limiters},
    routes,
    routings::app_routing::{AppRouting, RouterConfig},
    views::{SiteConfig, Views},
};

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("{0} must be set")]
    MissingEnv(&'static str),
    #[error("{name} is invalid: {reason}")]
    InvalidEnv { name: &'static str, reason: String },
    #[error("database: {0}")]
    Database(#[from] DbErr),
    #[error("{0}")]
    App(#[from] domain::Error),
    #[error("locale file: {0}")]
    Locale(#[from] toml::de::Error),
    #[error("logging: {0}")]
    Logging(String),
    #[error("cannot bind {addr}: {source}")]
    Bind {
        addr: String,
        source: std::io::Error,
    },
    #[error("server: {0}")]
    Serve(std::io::Error),
}

fn required_env(name: &'static str) -> Result<String, StartupError> {
    std::env::var(name).map_err(|_| StartupError::MissingEnv(name))
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn load_env_files() {
    // `.env` first; otherwise the per-environment file. Real environment
    // variables always win over both.
    if dotenvy::dotenv().is_err() {
        let env = optional_env("APP_ENV").unwrap_or_else(|| "dev".to_owned());
        // A missing file is normal in production, where the env comes from
        // the container; nothing to report.
        let _ = dotenvy::from_filename(format!("environments/.env.{env}"));
    }
}

const DEFAULT_LOG_FILTER: &str = "info,sea_orm=warn,sqlx=warn";

fn init_tracing() -> Result<(), StartupError> {
    // An empty RUST_LOG (common in copied .env files) parses as a valid filter
    // that hides even WARN, which would silence the fallback warnings below.
    let requested = optional_env("RUST_LOG");
    let parsed = requested
        .as_deref()
        .map(tracing_subscriber::EnvFilter::try_new);
    let filter = match &parsed {
        Some(Ok(f)) => f.clone(),
        _ => tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER),
    };
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    let result = if optional_env("LOG_FORMAT").as_deref() == Some("json") {
        builder.json().try_init()
    } else {
        builder.try_init()
    };
    result.map_err(|e| StartupError::Logging(e.to_string()))?;
    if let Some(Err(e)) = parsed {
        warn!(error = %e, "RUST_LOG is invalid; using {DEFAULT_LOG_FILTER}");
    }
    Ok(())
}

/// The public origin. Reset links point at it and it is the origin our own forms
/// post from, so it must be what the browser shows in the address bar.
fn app_url() -> String {
    optional_env("APP_URL").unwrap_or_else(|| {
        warn!(
            "APP_URL not set; assuming http://localhost:5173 for reset links and the Origin check"
        );
        DEFAULT_APP_URL.to_owned()
    })
}

/// The emails' wording, from the locale file; signed with the site name.
fn email_texts(locale: &Locale, site_name: &str) -> EmailTexts {
    EmailTexts {
        site_name: site_name.to_owned(),
        reset_subject: locale.text("email.reset_subject"),
        reset_body: locale.text("email.reset_body"),
        inquiry_subject: locale.text("email.inquiry_subject"),
        inquiry_body: locale.text("email.inquiry_body"),
        inquiry_note: locale.text("email.inquiry_note"),
    }
}

fn email_service(
    app_url: String,
    texts: EmailTexts,
) -> Result<Arc<dyn EmailService>, StartupError> {
    let Some(host) = optional_env("SMTP_HOST") else {
        warn!("SMTP_HOST not set; password-reset emails will be written to the log, not sent");
        return Ok(Arc::new(LogEmailService::new(app_url)));
    };

    let port = match optional_env("SMTP_PORT") {
        None => 587,
        Some(p) => p
            .parse()
            .map_err(|e: std::num::ParseIntError| StartupError::InvalidEnv {
                name: "SMTP_PORT",
                reason: e.to_string(),
            })?,
    };
    let credentials = match (optional_env("SMTP_USERNAME"), optional_env("SMTP_PASSWORD")) {
        (Some(user), Some(pass)) => Some((user, pass)),
        (None, None) => None,
        _ => {
            return Err(StartupError::InvalidEnv {
                name: "SMTP_USERNAME/SMTP_PASSWORD",
                reason: "set both or neither".into(),
            })
        }
    };
    let from = optional_env("FROM_EMAIL").unwrap_or_else(|| "no-reply@agri-connect.vn".to_owned());

    info!(%host, port, "sending email through SMTP");
    Ok(Arc::new(SmtpEmailService::new(SmtpConfig {
        host,
        port,
        credentials,
        from,
        app_url,
        texts,
    })?))
}

/// `CORS_ORIGINS` (comma-separated) and `HTTPS_ENABLED`. A malformed origin
/// fails startup: silently dropping it would block that site's browsers with
/// no hint why.
fn router_config(app_url: &str, limiters: Limiters) -> Result<RouterConfig, StartupError> {
    let cors_list: Vec<String> = optional_env("CORS_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    let cors_origins = cors_list
        .iter()
        .map(|s| {
            s.parse().map_err(|_| StartupError::InvalidEnv {
                name: "CORS_ORIGINS",
                reason: format!("{s:?} is not a valid header value"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let https = optional_env("HTTPS_ENABLED").as_deref() == Some("true");

    let mut allowed_origins = Vec::new();
    for url in std::iter::once(app_url).chain(cors_list.iter().map(String::as_str)) {
        let origin = origin_of(url).ok_or_else(|| StartupError::InvalidEnv {
            name: "APP_URL/CORS_ORIGINS",
            reason: format!("{url:?} is not scheme://host[:port]"),
        })?;
        allowed_origins.push(origin);
    }

    let trusted_proxy_hops = match optional_env("TRUSTED_PROXY_HOPS") {
        None => 0,
        Some(v) => v
            .parse()
            .map_err(|e: std::num::ParseIntError| StartupError::InvalidEnv {
                name: "TRUSTED_PROXY_HOPS",
                reason: e.to_string(),
            })?,
    };

    Ok(RouterConfig {
        cors_origins,
        https,
        trusted_proxy_hops,
        allowed_origins,
        limiters,
    })
}

/// `SITE_NAME` / `SITE_DESCRIPTION`, shown in every page's title and meta
/// tags, and the help lines the footer offers (`SITE_SUPPORT_PHONE`,
/// `SITE_SUPPORT_ZALO_URL`, `SITE_SUPPORT_EMAIL`).
fn site_config() -> SiteConfig {
    let defaults = SiteConfig::default();
    SiteConfig {
        name: optional_env("SITE_NAME").unwrap_or(defaults.name),
        description: optional_env("SITE_DESCRIPTION"),
        support_phone: optional_env("SITE_SUPPORT_PHONE"),
        support_zalo_url: optional_env("SITE_SUPPORT_ZALO_URL"),
        support_email: optional_env("SITE_SUPPORT_EMAIL"),
    }
}

/// Where uploads go and how big they may be.
#[derive(Clone)]
pub struct Uploads {
    pub storage: Arc<dyn FileStorage>,
    pub max_bytes: usize,
}

/// `GCS_BUCKET` (production) or else `UPLOAD_DIR`; `UPLOAD_MAX_BYTES`.
fn uploads() -> Result<Uploads, StartupError> {
    let storage: Arc<dyn FileStorage> = match optional_env("GCS_BUCKET") {
        Some(bucket) => {
            info!(%bucket, "uploads are stored in Google Cloud Storage");
            Arc::new(ObjectFileStorage::gcs(&bucket)?)
        }
        None => {
            let dir = optional_env("UPLOAD_DIR").unwrap_or_else(|| DEFAULT_UPLOAD_DIR.to_owned());
            warn!(%dir, "GCS_BUCKET not set; uploads are stored on local disk");
            Arc::new(ObjectFileStorage::local(std::path::Path::new(&dir))?)
        }
    };
    let max_bytes =
        match optional_env("UPLOAD_MAX_BYTES") {
            None => DEFAULT_UPLOAD_MAX_BYTES,
            Some(v) => v.parse().ok().filter(|&n: &usize| n > 0).ok_or_else(|| {
                StartupError::InvalidEnv {
                    name: "UPLOAD_MAX_BYTES",
                    reason: format!("{v:?} is not a positive byte count"),
                }
            })?,
        };
    Ok(Uploads { storage, max_bytes })
}

/// `REFRESH_TOKEN_DAYS`, how long an unused browser session lasts.
fn refresh_days() -> Result<i64, StartupError> {
    match optional_env("REFRESH_TOKEN_DAYS") {
        None => Ok(REFRESH_TOKEN_DEFAULT_DAYS),
        Some(v) => {
            v.parse()
                .ok()
                .filter(|&d: &i64| d > 0)
                .ok_or_else(|| StartupError::InvalidEnv {
                    name: "REFRESH_TOKEN_DAYS",
                    reason: format!("{v:?} is not a positive number of days"),
                })
        }
    }
}

/// The write pool and the read pool. They are the same pool unless
/// `DATABASE_READ_URL` is set; keeping two handles now means a read replica
/// can be added later without touching the wiring.
#[derive(Clone)]
pub struct Databases {
    pub primary: Arc<DatabaseConnection>,
    /// Only for repositories that never write. A replica lags the primary, so
    /// anything that reads its own write must stay on `primary`.
    pub read: Arc<DatabaseConnection>,
}

impl Databases {
    pub fn single(db: DatabaseConnection) -> Self {
        let db = Arc::new(db);
        Self {
            primary: db.clone(),
            read: db,
        }
    }
}

async fn connect(url: String) -> Result<DatabaseConnection, StartupError> {
    let mut options = ConnectOptions::new(url);
    options.max_connections(DB_MAX_CONNECTIONS);
    Ok(Database::connect(options).await?)
}

/// Wires every use case. Public so integration tests can build the exact
/// production graph on a test database.
pub fn build_state(
    dbs: Databases,
    token_service: Arc<dyn TokenService>,
    email: Arc<dyn EmailService>,
    site: SiteConfig,
    uploads: Uploads,
    // REFRESH_TOKEN_DAYS.
    refresh_days: i64,
) -> Result<AppState, StartupError> {
    let db = dbs.primary;
    let business_repo = Arc::new(BusinessRepository::new(Arc::clone(&db)));
    let product_repo = Arc::new(ProductRepository::new(Arc::clone(&db)));
    let category_repo = Arc::new(CategoryRepository::new(Arc::clone(&db)));
    let inquiry_repo = Arc::new(InquiryRepository::new(Arc::clone(&db)));
    let location_repo = Arc::new(LocationRepository::new(Arc::clone(&db)));
    let review_repo = Arc::new(ReviewRepository::new(Arc::clone(&db)));
    let service_repo = Arc::new(ServiceRepository::new(Arc::clone(&db)));
    let call_repo = Arc::new(CallRepository::new(Arc::clone(&db)));
    let file_repo = Arc::new(FileRepository::new(Arc::clone(&db)));
    // Read-only, and checked on many requests: the natural first user of a replica.
    let permission_repo = Arc::new(PermissionRepository::new(Arc::clone(&dbs.read)));
    let role_repo = Arc::new(RoleRepository::new(Arc::clone(&db)));
    let user_repo = Arc::new(UserRepository::new(Arc::clone(&db)));
    let audit_log_repo = Arc::new(AuditLogRepository::new(Arc::clone(&db)));
    let catalogue_repo = Arc::new(CatalogueRepository::new(Arc::clone(&db)));
    let demo_repo = Arc::new(DemoDataRepository::new(Arc::clone(&db)));
    // Readiness is about the primary: without it nothing can be written.
    let health_repo = Arc::new(HealthRepository::new(Arc::clone(&db)));

    let password_hasher: Arc<dyn PasswordHasher> = Arc::new(BcryptPasswordHasher);
    // The inbox the "new inquiry" email links to, from the route table.
    let inbox_path = routes::url("provider_inquiries", &[])
        .map_err(|e| domain::Error::internal(format!("route provider_inquiries: {e}")))?;
    // Subscribers run in this order for every event. Webhooks would be added
    // here.
    let events: Arc<dyn EventBus> = Arc::new(InProcessEventBus::new(vec![
        Arc::new(AuditLogSubscriber::new(audit_log_repo)),
        Arc::new(FileCleanupSubscriber::new(
            product_repo.clone(),
            service_repo.clone(),
            file_repo.clone(),
            uploads.storage.clone(),
        )),
        Arc::new(InquiryNotificationSubscriber::new(
            inquiry_repo.clone(),
            email.clone(),
            inbox_path,
        )),
    ]));
    let images = Arc::new(RasterImageProcessor::new(ImageRules {
        max_pixels: IMAGE_MAX_PIXELS,
        main_max_side: IMAGE_MAIN_MAX_SIDE,
        thumb_max_side: IMAGE_THUMB_MAX_SIDE,
        jpeg_quality: IMAGE_JPEG_QUALITY,
    }));

    let media = MediaUseCase::new(MediaDeps {
        products: product_repo.clone(),
        services: service_repo.clone(),
        files: file_repo,
        storage: uploads.storage,
        images,
        events: events.clone(),
        max_upload_bytes: uploads.max_bytes,
    });

    let usecases = UseCases {
        explore: ExploreUseCase::new(ExploreDeps {
            business_repo: business_repo.clone(),
            category_repo: category_repo.clone(),
            inquiry_repo: inquiry_repo.clone(),
            location_repo: location_repo.clone(),
            product_repo: product_repo.clone(),
            service_repo: service_repo.clone(),
            call_repo: call_repo.clone(),
            events: events.clone(),
        }),
        health: HealthUseCase::new(health_repo),
        media: media.clone(),
        product: ProductUseCase::new(product_repo.clone()),
        admin: AdminUseCase::new(AdminDeps {
            business_repo: business_repo.clone(),
            user_repo: user_repo.clone(),
            category_repo,
            inquiry_repo: inquiry_repo.clone(),
            location_repo,
            review_repo: review_repo.clone(),
            product_repo: product_repo.clone(),
            role_repo: role_repo.clone(),
            service_repo: service_repo.clone(),
            events: events.clone(),
            password_hasher: password_hasher.clone(),
        }),
        auth: AuthUseCase::new(AuthDeps {
            user_repo: user_repo.clone(),
            permission_repo: permission_repo.clone(),
            refresh_repo: Arc::new(RefreshTokenRepository::new(Arc::clone(&db))),
            email,
            password_hasher,
            token_svc: token_service.clone(),
            refresh_days,
        }),
        permission: PermissionUseCase::new(PermissionDeps {
            // The primary, not the read pool: the role page reads its own
            // grant changes right after saving them.
            permission_repo: Arc::new(PermissionRepository::new(Arc::clone(&db))),
            user_repo: user_repo.clone(),
            role_repo: role_repo.clone(),
            grants_repo: role_repo.clone(),
            events: events.clone(),
        }),
        review: ReviewUseCase::new(ReviewDeps {
            review_repo,
            inquiry_repo: inquiry_repo.clone(),
            business_repo: business_repo.clone(),
            events: events.clone(),
        }),
        provider_dashboard: ProviderDashboardUseCase::new(ProviderDashboardDeps {
            business_repo: business_repo.clone(),
            product_repo: product_repo.clone(),
            service_repo,
            call_repo: call_repo.clone(),
            inquiry_repo: inquiry_repo.clone(),
            media: media.clone(),
            events,
        }),
        setup: SetupUseCase::new(
            catalogue_repo,
            demo_repo,
            business_repo.clone(),
            user_repo.clone(),
            media,
        ),
        stats: StatsUseCase::new(StatsDeps {
            business_repo,
            product_repo,
            user_repo,
            call_repo: call_repo.clone(),
            inquiry_repo,
        }),
    };

    let locales = Arc::new(Locales::load()?);
    Ok(AppState {
        usecases: Arc::new(usecases),
        token_service,
        views: Arc::new(Views::new(site, Arc::clone(&locales))?),
        locales,
    })
}

/// Resolves on SIGTERM (what `docker stop` sends) or Ctrl-C. In-flight
/// requests then finish before the process exits; without this a deploy cuts
/// off whatever the server was doing.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            warn!(error = %e, "cannot listen for Ctrl-C");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                warn!(error = %e, "cannot listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    info!("shutdown signal received; draining in-flight requests");
}

pub async fn start() -> Result<(), StartupError> {
    load_env_files();
    init_tracing()?;

    let primary = connect(required_env("DATABASE_URL")?).await?;

    // A failure here stops startup: serving on a schema that does not match
    // the code corrupts data instead of failing loudly.
    Migrator::up(&primary, None).await?;

    let dbs = match optional_env("DATABASE_READ_URL") {
        Some(url) => {
            info!("reads that tolerate lag use DATABASE_READ_URL");
            Databases {
                primary: Arc::new(primary),
                read: Arc::new(connect(url).await?),
            }
        }
        None => Databases::single(primary),
    };

    let token_service: Arc<dyn TokenService> =
        Arc::new(JwtTokenService::new(&required_env("JWT_SECRET")?)?);
    let app_url = app_url();
    let site = site_config();
    // Emails go out in the default language: whom they reach has not chosen one.
    let texts = email_texts(Locales::load()?.get(Lang::DEFAULT), &site.name);
    let state = build_state(
        dbs,
        token_service,
        email_service(app_url.clone(), texts)?,
        site,
        uploads()?,
        refresh_days()?,
    )?;

    // Roles and permissions must exist before the first request, or the first
    // registration fails on a missing role.
    state.usecases.setup.seed_catalogue(&CATALOGUE).await?;

    let sessions = state.usecases.clone();
    tokio::spawn(async move {
        // Expired refresh tokens are dead weight; nothing reads them again.
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tick.tick().await;
            match sessions.auth.purge_expired_sessions().await {
                Ok(0) => {}
                Ok(n) => info!(deleted = n, "purged expired refresh tokens"),
                Err(e) => warn!(error = %e, "could not purge expired refresh tokens"),
            }
        }
    });

    let limiters = Limiters::in_process();
    let pruned = limiters.clone();
    tokio::spawn(async move {
        // Bounded memory: one entry per client seen in the last few minutes.
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            tick.tick().await;
            pruned.forget_idle();
        }
    });

    let router = AppRouting::build(state, router_config(&app_url, limiters)?);
    let addr = optional_env("BIND_ADDR").unwrap_or_else(|| DEFAULT_BIND_ADDR.to_owned());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|source| StartupError::Bind {
            addr: addr.clone(),
            source,
        })?;

    info!("Agri-Connect listening on http://{addr}");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(StartupError::Serve)?;
    info!("shut down cleanly");
    Ok(())
}

/// `agri-connect healthcheck [live|ready]`: probes the running server on
/// BIND_ADDR's port. Exists because the runtime image is distroless (no curl
/// or shell), and both the container healthcheck and the deploy gate need one.
pub async fn healthcheck(probe: &str) -> Result<(), String> {
    let path = match probe {
        "live" => "/health/live",
        "ready" => "/health/ready",
        other => return Err(format!("unknown probe {other:?}; use live or ready")),
    };
    let bind = optional_env("BIND_ADDR").unwrap_or_else(|| DEFAULT_BIND_ADDR.to_owned());
    let port = bind
        .rsplit(':')
        .next()
        .ok_or_else(|| format!("BIND_ADDR {bind:?} has no port"))?;
    let uri: axum::http::Uri = format!("http://127.0.0.1:{port}{path}")
        .parse()
        .map_err(|e| format!("probe uri: {e}"))?;

    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build_http::<axum::body::Body>();
    let response = tokio::time::timeout(std::time::Duration::from_secs(5), client.get(uri))
        .await
        .map_err(|_| "no answer within 5 s".to_owned())?
        .map_err(|e| format!("request failed: {e}"))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("{path} answered {}", response.status()))
    }
}
