use std::sync::Arc;

use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, Statement};
use tracing::{info, warn};

use crate::{
    app_state::{AppState, UseCases},
    application::{
        ports::{EmailService as EmailServicePort, PasswordHasher, TokenService},
        usecases::{
            admin_usecase::AdminUseCase, explore_usecase::ExploreUseCase,
            auth_usecase::AuthUseCase, permission_usecase::PermissionUseCase,
            product_usecase::ProductUseCase,
            provider_dashboard_usecase::ProviderDashboardUseCase,
            setup_usecase::SetupUseCase, stats_usecase::StatsUseCase,
        },
    },
    domain::repositories::{
        audit_log_repository_trait, business_repository_trait, category_repository_trait,
        inquiry_repository_trait, location_repository_trait, permission_repository_trait,
        product_repository_trait, review_repository_trait, role_repository_trait,
        seed_runner_trait, service_repository_trait, user_repository_trait,
    },
    infrastructure::{
        bcrypt_password_hasher::BcryptPasswordHasher,
        email_service::EmailService,
        jwt_token_service::JwtTokenService,
        repositories::{
            audit_log_repository::AuditLogRepository,
            business_repository::BusinessRepository, category_repository::CategoryRepository,
            inquiry_repository::InquiryRepository, location_repository::LocationRepository,
            permission_repository::PermissionRepository, product_repository::ProductRepository,
            review_repository::ReviewRepository, role_repository::RoleRepository,
            service_repository::ServiceRepository, user_repository::UserRepository,
        },
        seed_runner::SeedRunner,
    },
    routings::app_routing::AppRouting,
};

async fn tables_exist(db: &sea_orm::DatabaseConnection) -> bool {
    db.execute(Statement::from_string(
        db.get_database_backend(),
        "SELECT 1 FROM roles LIMIT 1",
    ))
    .await
    .is_ok()
}

async fn run_migrations(db: &sea_orm::DatabaseConnection) {
    Migrator::up(db, None)
        .await
        .expect("Database migration failed");

    if tables_exist(db).await {
        return;
    }

    // seaql_migrations is out of sync with reality — reset and retry.
    warn!("Migration tracking table is stale (tables missing). Resetting migration state and re-running.");
    db.execute(Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS seaql_migrations",
    ))
    .await
    .expect("Failed to drop stale seaql_migrations table");

    Migrator::up(db, None)
        .await
        .expect("Database migration failed after state reset");

    if !tables_exist(db).await {
        panic!("Database tables still missing after migration reset — check migration files.");
    }

    info!("Database migration recovery succeeded.");
}

pub async fn start() {
    // Try .env first, then fall back to the environment-specific file.
    if dotenvy::dotenv().is_err() {
        let env = std::env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());
        dotenvy::from_filename(format!("environments/.env.{env}")).ok();
    }

    let log_format = std::env::var("LOG_FORMAT").unwrap_or_default();
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,explore=debug,sea_orm=warn".into());

    if log_format == "json" {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let mut opt = ConnectOptions::new(database_url);
    opt.max_connections(crate::constants::DB_MAX_CONNECTIONS);

    let db = Database::connect(opt)
        .await
        .expect("Failed to connect to PostgreSQL");

    run_migrations(&db).await;

    // ── Shared DB connection ─────────────────────────────────────────────────
    let db = Arc::new(db);

    // ── Repositories (concrete implementations) ──────────────────────────────
    let business_repo: Arc<dyn business_repository_trait::BusinessRepositoryTrait + Send + Sync> =
        Arc::new(BusinessRepository::new(Arc::clone(&db)));
    let product_repo: Arc<dyn product_repository_trait::ProductRepositoryTrait + Send + Sync> =
        Arc::new(ProductRepository::new(Arc::clone(&db)));
    let category_repo: Arc<dyn category_repository_trait::CategoryRepositoryTrait + Send + Sync> =
        Arc::new(CategoryRepository::new(Arc::clone(&db)));
    let inquiry_repo: Arc<dyn inquiry_repository_trait::InquiryRepositoryTrait + Send + Sync> =
        Arc::new(InquiryRepository::new(Arc::clone(&db)));
    let location_repo: Arc<dyn location_repository_trait::LocationRepositoryTrait + Send + Sync> =
        Arc::new(LocationRepository::new(Arc::clone(&db)));
    let review_repo: Arc<dyn review_repository_trait::ReviewRepositoryTrait + Send + Sync> =
        Arc::new(ReviewRepository::new(Arc::clone(&db)));
    let service_repo: Arc<dyn service_repository_trait::ServiceRepositoryTrait + Send + Sync> =
        Arc::new(ServiceRepository::new(Arc::clone(&db)));
    let permission_repo: Arc<dyn permission_repository_trait::PermissionRepositoryTrait + Send + Sync> =
        Arc::new(PermissionRepository::new(Arc::clone(&db)));
    let role_repo: Arc<dyn role_repository_trait::RoleRepositoryTrait + Send + Sync> =
        Arc::new(RoleRepository::new(Arc::clone(&db)));
    let user_repo: Arc<dyn user_repository_trait::UserRepositoryTrait + Send + Sync> =
        Arc::new(UserRepository::new(Arc::clone(&db)));
    let seed_runner: Arc<dyn seed_runner_trait::SeedRunnerTrait + Send + Sync> =
        Arc::new(SeedRunner::new(Arc::clone(&db)));
    let audit_log_repo: Arc<dyn audit_log_repository_trait::AuditLogRepositoryTrait + Send + Sync> =
        Arc::new(AuditLogRepository::new(Arc::clone(&db)));

    // ── Infrastructure services (ports) ──────────────────────────────────────
    // JWT keys must be initialised before JwtTokenService is used.
    crate::middleware::auth::init_jwt_keys();

    // Traits already declare `Send + Sync` as supertraits, so `Arc<dyn Trait>`
    // is sufficient — no need to repeat the bounds in the type annotation.
    let password_hasher: Arc<dyn PasswordHasher> = Arc::new(BcryptPasswordHasher);
    let token_svc:       Arc<dyn TokenService>   = Arc::new(JwtTokenService);
    let email_svc:       Arc<dyn EmailServicePort> = Arc::new(EmailService::new());

    // ── Use cases (business logic, depend on repo traits) ────────────────────
    let explore_usecase = ExploreUseCase::new(
        Arc::clone(&business_repo),
        Arc::clone(&category_repo),
        Arc::clone(&inquiry_repo),
        Arc::clone(&location_repo),
        Arc::clone(&product_repo),
    );
    let product_usecase = ProductUseCase::new(Arc::clone(&product_repo));
    let admin_usecase = AdminUseCase::new(
        Arc::clone(&business_repo),
        Arc::clone(&user_repo),
        Arc::clone(&category_repo),
        Arc::clone(&inquiry_repo),
        Arc::clone(&location_repo),
        Arc::clone(&review_repo),
        Arc::clone(&product_repo),
        Arc::clone(&role_repo),
        Arc::clone(&service_repo),
        Arc::clone(&audit_log_repo),
        Arc::clone(&permission_repo),
        Arc::clone(&password_hasher),
    );
    let auth_usecase = AuthUseCase::new(
        Arc::clone(&user_repo),
        Arc::clone(&business_repo),
        Arc::clone(&role_repo),
        Arc::clone(&permission_repo),
        Arc::clone(&email_svc),
        Arc::clone(&password_hasher),
        Arc::clone(&token_svc),
    );
    let permission_usecase = PermissionUseCase::new(
        Arc::clone(&permission_repo),
        Arc::clone(&user_repo),
    );
    let provider_dashboard_usecase = ProviderDashboardUseCase::new(
        Arc::clone(&business_repo),
        Arc::clone(&product_repo),
        Arc::clone(&permission_repo),
    );
    let setup_usecase = SetupUseCase::new(
        Arc::clone(&seed_runner),
        Arc::clone(&business_repo),
        Arc::clone(&user_repo),
    );
    let stats_usecase = StatsUseCase::new(
        Arc::clone(&business_repo),
        Arc::clone(&product_repo),
        Arc::clone(&user_repo),
    );

    // ── App state ─────────────────────────────────────────────────────────────
    let app_state = AppState {
        usecases: UseCases {
            explore: explore_usecase,
            product: product_usecase,
            admin: admin_usecase,
            auth: auth_usecase,
            permission: permission_usecase,
            provider_dashboard: provider_dashboard_usecase,
            setup: setup_usecase,
            stats: stats_usecase,
        },
    };

    // ── Router ────────────────────────────────────────────────────────────────
    let router = AppRouting::build(app_state);

    let listener = tokio::net::TcpListener::bind(crate::constants::SERVER_BIND_ADDR)
        .await
        .expect("Failed to bind server address");

    info!(
        "Agri-Connect backend listening on http://{}",
        crate::constants::SERVER_BIND_ADDR
    );
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .unwrap();
}
