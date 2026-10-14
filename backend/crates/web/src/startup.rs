//! Configuration, dependency wiring and server startup. The only place that
//! reads environment variables or names concrete adapters.

use std::{net::SocketAddr, sync::Arc};

use application::{
    catalogue::CATALOGUE,
    ports::{EmailService, PasswordHasher, TokenService},
    usecases::{
        admin_usecase::{AdminDeps, AdminUseCase},
        auth_usecase::AuthUseCase,
        explore_usecase::ExploreUseCase,
        health_usecase::HealthUseCase,
        permission_usecase::PermissionUseCase,
        product_usecase::ProductUseCase,
        provider_dashboard_usecase::ProviderDashboardUseCase,
        setup_usecase::SetupUseCase,
        stats_usecase::StatsUseCase,
    },
};
use infrastructure::{
    bcrypt_password_hasher::BcryptPasswordHasher,
    catalogue_repository::CatalogueRepository,
    demo_data_repository::DemoDataRepository,
    email_service::{LogEmailService, SmtpConfig, SmtpEmailService},
    jwt_token_service::JwtTokenService,
    repositories::{
        audit_log_repository::AuditLogRepository, business_repository::BusinessRepository,
        category_repository::CategoryRepository, health_repository::HealthRepository,
        inquiry_repository::InquiryRepository, location_repository::LocationRepository,
        permission_repository::PermissionRepository, product_repository::ProductRepository,
        review_repository::ReviewRepository, role_repository::RoleRepository,
        service_repository::ServiceRepository, user_repository::UserRepository,
    },
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use tracing::{info, warn};

use crate::{
    app_state::{AppState, UseCases},
    constants::{DB_MAX_CONNECTIONS, DEFAULT_BIND_ADDR},
    locale::Locale,
    routings::app_routing::{AppRouting, RouterConfig},
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

fn email_service() -> Result<Arc<dyn EmailService>, StartupError> {
    let app_url = optional_env("APP_URL").unwrap_or_else(|| {
        warn!("APP_URL not set; password-reset links will point at http://localhost:5173");
        "http://localhost:5173".to_owned()
    });

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
    })?))
}

/// `CORS_ORIGINS` (comma-separated) and `HTTPS_ENABLED`. A malformed origin
/// fails startup: silently dropping it would block that site's browsers with
/// no hint why.
fn router_config() -> Result<RouterConfig, StartupError> {
    let cors_origins = optional_env("CORS_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse().map_err(|_| StartupError::InvalidEnv {
                name: "CORS_ORIGINS",
                reason: format!("{s:?} is not a valid header value"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RouterConfig {
        cors_origins,
        https: optional_env("HTTPS_ENABLED").as_deref() == Some("true"),
    })
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
) -> Result<AppState, StartupError> {
    let db = dbs.primary;
    let business_repo = Arc::new(BusinessRepository::new(Arc::clone(&db)));
    let product_repo = Arc::new(ProductRepository::new(Arc::clone(&db)));
    let category_repo = Arc::new(CategoryRepository::new(Arc::clone(&db)));
    let inquiry_repo = Arc::new(InquiryRepository::new(Arc::clone(&db)));
    let location_repo = Arc::new(LocationRepository::new(Arc::clone(&db)));
    let review_repo = Arc::new(ReviewRepository::new(Arc::clone(&db)));
    let service_repo = Arc::new(ServiceRepository::new(Arc::clone(&db)));
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

    let usecases = UseCases {
        explore: ExploreUseCase::new(
            business_repo.clone(),
            category_repo.clone(),
            inquiry_repo.clone(),
            location_repo.clone(),
            product_repo.clone(),
        ),
        health: HealthUseCase::new(health_repo),
        product: ProductUseCase::new(product_repo.clone()),
        admin: AdminUseCase::new(AdminDeps {
            business_repo: business_repo.clone(),
            user_repo: user_repo.clone(),
            category_repo,
            inquiry_repo,
            location_repo,
            review_repo,
            product_repo: product_repo.clone(),
            role_repo,
            service_repo,
            audit_log_repo,
            password_hasher: password_hasher.clone(),
        }),
        auth: AuthUseCase::new(
            user_repo.clone(),
            business_repo.clone(),
            email,
            password_hasher,
            token_service.clone(),
        ),
        permission: PermissionUseCase::new(permission_repo.clone(), user_repo.clone()),
        provider_dashboard: ProviderDashboardUseCase::new(
            business_repo.clone(),
            product_repo.clone(),
            permission_repo,
        ),
        setup: SetupUseCase::new(
            catalogue_repo,
            demo_repo,
            business_repo.clone(),
            user_repo.clone(),
        ),
        stats: StatsUseCase::new(business_repo, product_repo, user_repo),
    };

    Ok(AppState {
        usecases: Arc::new(usecases),
        token_service,
        locale: Arc::new(Locale::vi()?),
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
    let state = build_state(dbs, token_service, email_service()?)?;

    // Roles and permissions must exist before the first request, or the first
    // registration fails on a missing role.
    state.usecases.setup.seed_catalogue(&CATALOGUE).await?;

    let router = AppRouting::build(state, router_config()?);
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
