//! Configuration, dependency wiring and server startup. The only place that
//! reads environment variables or names concrete adapters.

use std::{net::SocketAddr, sync::Arc};

use application::{
    catalogue::CATALOGUE,
    constants::{
        DEFAULT_UPLOAD_MAX_BYTES, IMAGE_JPEG_QUALITY, IMAGE_MAIN_MAX_SIDE, IMAGE_MAX_PIXELS,
        IMAGE_THUMB_MAX_SIDE, REFRESH_TOKEN_DEFAULT_DAYS, SETUP_TOKEN_MIN_CHARS,
    },
    events::{
        AuditLogSubscriber, FileCleanupSubscriber, InProcessEventBus,
        InquiryNotificationSubscriber, JobNotificationSubscriber,
    },
    ports::{EmailService, EventBus, FileStorage, PasswordHasher, TokenService, ZaloGateway},
    settings::SettingsReader,
    usecases::{
        admin_usecase::{AdminDeps, AdminUseCase},
        alert_usecase::AlertUseCase,
        auth_usecase::{AuthDeps, AuthUseCase},
        coded_usecase::{CodedUseCase, ALERT_KIND_LIST, SECTOR_LIST},
        commodity_usecase::CommodityUseCase,
        demand_usecase::{DemandDeps, DemandReportUseCase},
        explore_usecase::{ExploreDeps, ExploreUseCase},
        health_usecase::HealthUseCase,
        job_usecase::JobUseCase,
        media_usecase::{MediaDeps, MediaUseCase},
        partner_usecase::{PartnerDeps, PartnerUseCase},
        permission_usecase::{PermissionDeps, PermissionUseCase},
        price_usecase::PriceUseCase,
        product_usecase::ProductUseCase,
        provider_dashboard_usecase::{ProviderDashboardDeps, ProviderDashboardUseCase},
        review_usecase::{ReviewDeps, ReviewUseCase},
        settings_usecase::SettingsUseCase,
        setup_usecase::SetupUseCase,
        stats_usecase::{StatsDeps, StatsUseCase},
        zalo_usecase::ZaloUseCase,
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
        admin_unit_repository::AdminUnitRepository,
        alert_repository::AlertRepository,
        audit_log_repository::AuditLogRepository,
        business_repository::BusinessRepository,
        call_repository::CallRepository,
        category_repository::CategoryRepository,
        coded_repository::{AlertKindRepository, SectorRepository},
        commodity_repository::CommodityRepository,
        demand_repository::DemandRepository,
        file_repository::FileRepository,
        health_repository::HealthRepository,
        inquiry_repository::InquiryRepository,
        job_repository::JobRepository,
        location_repository::LocationRepository,
        partner_repository::PartnerRepository,
        permission_repository::PermissionRepository,
        price_repository::PriceRepository,
        product_repository::ProductRepository,
        refresh_token_repository::RefreshTokenRepository,
        review_repository::ReviewRepository,
        role_repository::RoleRepository,
        service_repository::ServiceRepository,
        setting_repository::SettingRepository,
        user_repository::UserRepository,
    },
    zalo_gateway::ZaloGraphGateway,
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use tracing::{info, warn};

use crate::{
    app_state::{AppState, UseCases},
    constants::{
        DB_ACQUIRE_TIMEOUT_SECS, DB_MAX_CONNECTIONS, DB_STATEMENT_TIMEOUT_SECS, DEFAULT_APP_URL,
        DEFAULT_BIND_ADDR, DEFAULT_UPLOAD_DIR, EVENT_ROLL_UP_EVERY_SECS, HEALTHCHECK_TIMEOUT_SECS,
        RATE_LIMIT_PRUNE_EVERY_SECS, SESSION_PURGE_EVERY_SECS, SMTP_PORT_DEFAULT, ZALO_GRAPH_URL,
        ZALO_TIMEOUT_SECS,
    },
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

/// `APP_ENV=prod`: what must never be left to a development fallback
/// (reset links in the log, a setup page open to the first visitor) is then
/// required at startup.
fn is_production() -> bool {
    optional_env("APP_ENV").as_deref() == Some("prod")
}

/// `SETUP_TOKEN`: what the first-admin form must be given. Unset: the form is
/// open to whoever reaches it first, which production refuses (see `start`).
fn setup_token() -> Result<Option<String>, StartupError> {
    match optional_env("SETUP_TOKEN").map(|t| t.trim().to_owned()) {
        Some(t) if t.chars().count() < SETUP_TOKEN_MIN_CHARS => Err(StartupError::InvalidEnv {
            name: "SETUP_TOKEN",
            reason: format!("must be at least {SETUP_TOKEN_MIN_CHARS} characters"),
        }),
        token => Ok(token),
    }
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
        verify_subject: locale.text("email.verify_subject"),
        verify_body: locale.text("email.verify_body"),
        inquiry_subject: locale.text("email.inquiry_subject"),
        inquiry_body: locale.text("email.inquiry_body"),
        inquiry_note: locale.text("email.inquiry_note"),
        job_subject: locale.text("email.job_subject"),
        job_body: locale.text("email.job_body"),
    }
}

fn email_service(
    app_url: String,
    texts: EmailTexts,
) -> Result<Arc<dyn EmailService>, StartupError> {
    let Some(host) = optional_env("SMTP_HOST") else {
        if is_production() {
            return Err(StartupError::InvalidEnv {
                name: "SMTP_HOST",
                reason: "required in production: without it password-reset links are written \
                         to the log, where anyone who reads it can take an account over"
                    .into(),
            });
        }
        warn!("SMTP_HOST not set; password-reset emails will be written to the log, not sent");
        return Ok(Arc::new(LogEmailService::new(app_url)));
    };

    let port = match optional_env("SMTP_PORT") {
        None => SMTP_PORT_DEFAULT,
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

    let origin = |url: &str| {
        origin_of(url).ok_or_else(|| StartupError::InvalidEnv {
            name: "APP_URL/CORS_ORIGINS",
            reason: format!("{url:?} is not scheme://host[:port]"),
        })
    };
    let allowed_origins = vec![origin(app_url)?];
    let api_origins = cors_list
        .iter()
        .map(|url| origin(url))
        .collect::<Result<Vec<_>, _>>()?;

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
        api_origins,
        limiters,
    })
}

/// `SITE_NAME` / `SITE_DESCRIPTION`, shown in every page's title and meta
/// tags. The footer's help lines are settings (`application::settings`).
fn site_config() -> SiteConfig {
    let defaults = SiteConfig::default();
    SiteConfig {
        name: optional_env("SITE_NAME").unwrap_or(defaults.name),
        description: optional_env("SITE_DESCRIPTION"),
    }
}

/// `ZALO_APP_SECRET`: the Zalo app's secret key (developers.zalo.me), which
/// lets the server read a Mini App's location and phone tokens. Unset: the
/// Mini App asks for an area and a typed phone instead.
fn zalo_gateway() -> Result<Option<Arc<dyn ZaloGateway>>, StartupError> {
    let Some(secret) = optional_env("ZALO_APP_SECRET") else {
        info!("ZALO_APP_SECRET not set; the Mini App's location and phone tokens are not read");
        return Ok(None);
    };
    let gateway = ZaloGraphGateway::new(
        ZALO_GRAPH_URL.to_owned(),
        secret,
        std::time::Duration::from_secs(ZALO_TIMEOUT_SECS),
    )?;
    Ok(Some(Arc::new(gateway)))
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

/// A whole number of at least 1 from the environment, or `default`.
fn positive_env<T>(name: &'static str, default: T) -> Result<T, StartupError>
where
    T: std::str::FromStr + PartialOrd + From<u8>,
{
    match optional_env(name) {
        None => Ok(default),
        Some(v) => v
            .trim()
            .parse::<T>()
            .ok()
            .filter(|n| *n >= T::from(1))
            .ok_or_else(|| StartupError::InvalidEnv {
                name,
                reason: format!("{v:?} is not a whole number of at least 1"),
            }),
    }
}

/// The pool the server answers requests from: bounded, quick to say it is
/// full, and never busy with one statement for longer than a request lives.
async fn connect(url: String) -> Result<DatabaseConnection, StartupError> {
    let statement_timeout = positive_env("DB_STATEMENT_TIMEOUT_SECS", DB_STATEMENT_TIMEOUT_SECS)?;
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(positive_env("DB_MAX_CONNECTIONS", DB_MAX_CONNECTIONS)?)
        .acquire_timeout(std::time::Duration::from_secs(DB_ACQUIRE_TIMEOUT_SECS))
        .map_sqlx_postgres_opts(move |pg| {
            pg.options([("statement_timeout", format!("{statement_timeout}s"))])
        });
    Ok(Database::connect(options).await?)
}

/// Brings the schema up to date on a connection of its own, closed after:
/// building an index on a large table takes longer than any request may.
async fn migrate(url: String) -> Result<(), StartupError> {
    let mut options = ConnectOptions::new(url);
    options.max_connections(1);
    let db = Database::connect(options).await?;
    Migrator::up(&db, None).await?;
    db.close().await?;
    Ok(())
}

/// What `build_state` wires the use cases to: the outside world, chosen by
/// `run` from the environment, or by a test.
pub struct Wiring {
    pub dbs: Databases,
    pub token_service: Arc<dyn TokenService>,
    /// bcrypt at its default cost in production.
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub email: Arc<dyn EmailService>,
    /// Every language's templates, with the site identity. Building them is
    /// the costliest part of startup, so a test process builds them once.
    pub views: Arc<Views>,
    pub uploads: Uploads,
    /// The Zalo Graph API for Mini App tokens; `None` without ZALO_APP_SECRET.
    pub zalo: Option<Arc<dyn ZaloGateway>>,
    /// REFRESH_TOKEN_DAYS.
    pub refresh_days: i64,
    /// SETUP_TOKEN: the first-admin form asks for it when set.
    pub setup_token: Option<String>,
}

/// The templates of every language over `locales`, with the site identity.
pub fn views(site: SiteConfig, locales: Arc<Locales>) -> Result<Arc<Views>, StartupError> {
    Ok(Arc::new(Views::new(site, locales)?))
}

/// Wires every use case. Public so integration tests can build the exact
/// production graph on a test database.
pub fn build_state(wiring: Wiring) -> Result<AppState, StartupError> {
    let Wiring {
        dbs,
        token_service,
        password_hasher,
        email,
        views,
        uploads,
        zalo,
        refresh_days,
        setup_token,
    } = wiring;
    let db = dbs.primary;
    let business_repo = Arc::new(BusinessRepository::new(Arc::clone(&db)));
    let product_repo = Arc::new(ProductRepository::new(Arc::clone(&db)));
    let category_repo = Arc::new(CategoryRepository::new(Arc::clone(&db)));
    let commodity_repo = Arc::new(CommodityRepository::new(Arc::clone(&db)));
    let sector_repo = Arc::new(SectorRepository::new(Arc::clone(&db)));
    let alert_kind_repo = Arc::new(AlertKindRepository::new(Arc::clone(&db)));
    let inquiry_repo = Arc::new(InquiryRepository::new(Arc::clone(&db)));
    let location_repo = Arc::new(LocationRepository::new(Arc::clone(&db)));
    let review_repo = Arc::new(ReviewRepository::new(Arc::clone(&db)));
    let service_repo = Arc::new(ServiceRepository::new(Arc::clone(&db)));
    let call_repo = Arc::new(CallRepository::new(Arc::clone(&db)));
    let file_repo = Arc::new(FileRepository::new(Arc::clone(&db)));
    // On the primary, never the read pool: this is who the caller is and what
    // they may do on every request, and a lagging replica would keep a
    // revoked permission, a deactivated account or an ended session alive.
    let permission_repo = Arc::new(PermissionRepository::new(Arc::clone(&db)));
    let role_repo = Arc::new(RoleRepository::new(Arc::clone(&db)));
    let user_repo = Arc::new(UserRepository::new(Arc::clone(&db)));
    let refresh_repo = Arc::new(RefreshTokenRepository::new(Arc::clone(&db)));
    let audit_log_repo = Arc::new(AuditLogRepository::new(Arc::clone(&db)));
    let catalogue_repo = Arc::new(CatalogueRepository::new(Arc::clone(&db)));
    let admin_unit_repo = Arc::new(AdminUnitRepository::new(Arc::clone(&db)));
    let demo_repo = Arc::new(DemoDataRepository::new(Arc::clone(&db)));
    // Readiness is about the primary: without it nothing can be written.
    let health_repo = Arc::new(HealthRepository::new(Arc::clone(&db)));

    // The inbox the "new inquiry" email links to, from the route table.
    let inbox_path = routes::url("provider_inquiries", &[])
        .map_err(|e| domain::Error::internal(format!("route provider_inquiries: {e}")))?;
    // The jobs page the "a job near you" email links to.
    let jobs_path = routes::url("provider_jobs", &[])
        .map_err(|e| domain::Error::internal(format!("route provider_jobs: {e}")))?;
    let job_repo = Arc::new(JobRepository::new(Arc::clone(&db)));
    let setting_repo = Arc::new(SettingRepository::new(Arc::clone(&db)));
    // Every use case that follows a setting reads it through this, on each use.
    let settings = SettingsReader::new(setting_repo.clone());
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
        Arc::new(JobNotificationSubscriber::new(
            job_repo.clone(),
            email.clone(),
            settings.clone(),
            jobs_path,
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

    let demand_repo = Arc::new(DemandRepository::new(Arc::clone(&db)));
    let demand = DemandReportUseCase::new(DemandDeps {
        demand_repo: demand_repo.clone(),
        category_repo: category_repo.clone(),
        location_repo: location_repo.clone(),
        admin_unit_repo: admin_unit_repo.clone(),
        settings: settings.clone(),
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
            category_repo: category_repo.clone(),
            inquiry_repo: inquiry_repo.clone(),
            location_repo: location_repo.clone(),
            admin_unit_repo: admin_unit_repo.clone(),
            review_repo: review_repo.clone(),
            product_repo: product_repo.clone(),
            role_repo: role_repo.clone(),
            service_repo: service_repo.clone(),
            sector_repo: sector_repo.clone(),
            events: events.clone(),
            password_hasher: password_hasher.clone(),
            refresh_repo: refresh_repo.clone(),
            call_repo: call_repo.clone(),
            settings: settings.clone(),
        }),
        auth: AuthUseCase::new(AuthDeps {
            user_repo: user_repo.clone(),
            permission_repo: permission_repo.clone(),
            refresh_repo,
            email,
            password_hasher: password_hasher.clone(),
            token_svc: token_service.clone(),
            events: events.clone(),
            refresh_days,
            setup_token,
            settings: settings.clone(),
        }),
        permission: PermissionUseCase::new(PermissionDeps {
            permission_repo,
            user_repo: user_repo.clone(),
            role_repo: role_repo.clone(),
            grants_repo: role_repo.clone(),
            events: events.clone(),
        }),
        partner: PartnerUseCase::new(PartnerDeps {
            partner_repo: Arc::new(PartnerRepository::new(Arc::clone(&db))),
            user_repo: user_repo.clone(),
            category_repo: category_repo.clone(),
            admin_unit_repo: admin_unit_repo.clone(),
            demand_repo,
            demand: demand.clone(),
            password_hasher,
            settings: settings.clone(),
            events: events.clone(),
        }),
        job: JobUseCase::new(
            job_repo,
            category_repo.clone(),
            location_repo.clone(),
            events.clone(),
            settings.clone(),
        ),
        commodity: CommodityUseCase::new(
            commodity_repo.clone(),
            sector_repo.clone(),
            events.clone(),
        ),
        sector: CodedUseCase::new(SECTOR_LIST, sector_repo.clone(), events.clone()),
        alert_kind: CodedUseCase::new(ALERT_KIND_LIST, alert_kind_repo.clone(), events.clone()),
        alert: AlertUseCase::new(
            Arc::new(AlertRepository::new(Arc::clone(&db))),
            admin_unit_repo.clone(),
            sector_repo,
            alert_kind_repo,
            events.clone(),
        ),
        price: PriceUseCase::new(
            Arc::new(PriceRepository::new(Arc::clone(&db))),
            location_repo,
            commodity_repo.clone(),
            events.clone(),
        ),
        zalo: ZaloUseCase::new(zalo),
        demand,
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
            settings,
            events: events.clone(),
        }),
        setup: SetupUseCase::new(
            catalogue_repo,
            admin_unit_repo,
            demo_repo,
            business_repo.clone(),
            user_repo.clone(),
            media,
        ),
        settings: SettingsUseCase::new(setting_repo, events),
        stats: StatsUseCase::new(StatsDeps {
            business_repo,
            product_repo,
            user_repo,
            call_repo: call_repo.clone(),
        }),
    };

    Ok(AppState {
        usecases: Arc::new(usecases),
        token_service,
        locales: Arc::clone(views.locales()),
        views,
        // The router sets it from HTTPS_ENABLED.
        cookies: crate::session::CookiePolicy::default(),
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

    let database_url = required_env("DATABASE_URL")?;
    // A failure here stops startup: serving on a schema that does not match
    // the code corrupts data instead of failing loudly.
    // As the schema's owner when MIGRATION_DATABASE_URL is set (production:
    // agri_owner), so the role that answers requests never needs the right
    // to change the schema.
    migrate(optional_env("MIGRATION_DATABASE_URL").unwrap_or_else(|| database_url.clone())).await?;
    let primary = connect(database_url).await?;

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
    let locales = Arc::new(Locales::load()?);
    // Emails go out in the default language: whom they reach has not chosen one.
    let texts = email_texts(locales.get(Lang::DEFAULT), &site.name);
    let state = build_state(Wiring {
        dbs,
        token_service,
        password_hasher: Arc::new(BcryptPasswordHasher::default()),
        email: email_service(app_url.clone(), texts)?,
        views: views(site, locales)?,
        uploads: uploads()?,
        zalo: zalo_gateway()?,
        refresh_days: refresh_days()?,
        setup_token: setup_token()?,
    })?;

    // Roles and permissions must exist before the first request, or the first
    // registration fails on a missing role.
    state.usecases.setup.seed_catalogue(&CATALOGUE).await?;
    // The demand report and partner areas place every point in a ward.
    state.usecases.setup.seed_admin_units().await?;

    // A public server with no admin and an open setup form belongs to the
    // first stranger who finds it.
    let auth = &state.usecases.auth;
    if is_production() && !auth.setup_token_required() && auth.needs_first_admin().await? {
        return Err(StartupError::InvalidEnv {
            name: "SETUP_TOKEN",
            reason: "required in production until the first admin exists".into(),
        });
    }

    let sessions = state.usecases.clone();
    tokio::spawn(async move {
        // Expired refresh tokens are dead weight; nothing reads them again.
        let mut tick =
            tokio::time::interval(std::time::Duration::from_secs(SESSION_PURGE_EVERY_SECS));
        loop {
            tick.tick().await;
            match sessions.auth.purge_expired_sessions().await {
                Ok(0) => {}
                Ok(n) => info!(
                    deleted = n,
                    "purged expired refresh tokens and emailed links"
                ),
                Err(e) => warn!(error = %e, "could not purge expired refresh tokens and links"),
            }
        }
    });

    let demand = state.usecases.clone();
    tokio::spawn(async move {
        // Taps and searches older than any report become weekly totals, and
        // job posts past keeping are deleted, so those tables stay the size
        // of a few months however long the server runs.
        let mut tick =
            tokio::time::interval(std::time::Duration::from_secs(EVENT_ROLL_UP_EVERY_SECS));
        loop {
            tick.tick().await;
            match demand.demand.roll_up_old_events(chrono::Utc::now()).await {
                Ok(0) => {}
                Ok(n) => info!(
                    deleted = n,
                    "old taps and searches counted into weekly totals"
                ),
                Err(e) => warn!(error = %e, "could not roll up old taps and searches"),
            }
            match demand.job.purge_old_posts(chrono::Utc::now()).await {
                Ok(0) => {}
                Ok(n) => info!(deleted = n, "old job posts deleted"),
                Err(e) => warn!(error = %e, "could not delete old job posts"),
            }
        }
    });

    let limiters = Limiters::in_process();
    let pruned = limiters.clone();
    let repeats = state.usecases.clone();
    tokio::spawn(async move {
        // Bounded memory: one entry per client seen in the last few minutes,
        // and per tap or search still inside its repeat window.
        let mut tick =
            tokio::time::interval(std::time::Duration::from_secs(RATE_LIMIT_PRUNE_EVERY_SECS));
        loop {
            tick.tick().await;
            pruned.forget_idle();
            repeats.explore.forget_old_taps();
            repeats.demand.forget_old_searches();
            repeats.auth.forget_old_failed_logins();
            repeats.job.forget_old_posters();
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
        "live" => crate::routes::HEALTH_LIVE_PATH,
        "ready" => crate::routes::HEALTH_READY_PATH,
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
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(HEALTHCHECK_TIMEOUT_SECS),
        client.get(uri),
    )
    .await
    .map_err(|_| format!("no answer within {HEALTHCHECK_TIMEOUT_SECS} s"))?
    .map_err(|e| format!("request failed: {e}"))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("{path} answered {}", response.status()))
    }
}
