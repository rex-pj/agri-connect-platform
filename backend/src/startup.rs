use std::sync::Arc;

use sea_orm::{ConnectOptions, Database};
use tracing::info;

use crate::{
    app_state::{AppState, UseCases},
    application::usecases::{
        admin_usecase::AdminUseCase,
        agri_connect_usecase::AgriConnectUseCase,
        auth_usecase::AuthUseCase,
        product_usecase::ProductUseCase,
        provider_dashboard_usecase::ProviderDashboardUseCase,
        role_usecase::RoleUseCase,
        setup_usecase::SetupUseCase,
        stats_usecase::StatsUseCase,
    },
    infrastructure::{
        email_service::EmailService,
        repositories::{
            business_repository::BusinessRepository,
            category_repository::CategoryRepository,
            inquiry_repository::InquiryRepository,
            product_repository::ProductRepository,
            review_repository::ReviewRepository,
            role_repository::RoleRepository,
            user_repository::UserRepository,
        },
    },
    routings::app_routing::AppRouting,
};

pub async fn start() {
    // Try .env first, then fall back to the environment-specific file.
    if dotenvy::dotenv().is_err() {
        let env = std::env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());
        dotenvy::from_filename(format!("environments/.env.{env}")).ok();
    }

    let log_format = std::env::var("LOG_FORMAT").unwrap_or_default();
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,agri_connect=debug,sea_orm=warn".into());

    if log_format == "json" {
        tracing_subscriber::fmt().with_env_filter(filter).json().init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let mut opt = ConnectOptions::new(database_url);
    opt.max_connections(crate::constants::DB_MAX_CONNECTIONS);

    let db = Database::connect(opt)
        .await
        .expect("Failed to connect to PostgreSQL");

    // ── Shared DB connection ─────────────────────────────────────────────────
    let db = Arc::new(db);

    // ── Repositories (concrete implementations) ──────────────────────────────
    let business_repo: Arc<dyn crate::domain::repositories::business_repository_trait::BusinessRepositoryTrait + Send + Sync> =
        Arc::new(BusinessRepository::new(Arc::clone(&db)));
    let product_repo: Arc<dyn crate::domain::repositories::product_repository_trait::ProductRepositoryTrait + Send + Sync> =
        Arc::new(ProductRepository::new(Arc::clone(&db)));
    let category_repo: Arc<dyn crate::domain::repositories::category_repository_trait::CategoryRepositoryTrait + Send + Sync> =
        Arc::new(CategoryRepository::new(Arc::clone(&db)));
    let inquiry_repo: Arc<dyn crate::domain::repositories::inquiry_repository_trait::InquiryRepositoryTrait + Send + Sync> =
        Arc::new(InquiryRepository::new(Arc::clone(&db)));
    let review_repo: Arc<dyn crate::domain::repositories::review_repository_trait::ReviewRepositoryTrait + Send + Sync> =
        Arc::new(ReviewRepository::new(Arc::clone(&db)));
    let role_repo: Arc<dyn crate::domain::repositories::role_repository_trait::RoleRepositoryTrait + Send + Sync> =
        Arc::new(RoleRepository::new(Arc::clone(&db)));
    let user_repo: Arc<dyn crate::domain::repositories::user_repository_trait::UserRepositoryTrait + Send + Sync> =
        Arc::new(UserRepository::new(Arc::clone(&db)));

    // ── Use cases (business logic, depend on repo traits) ────────────────────
    let agri_connect_usecase = AgriConnectUseCase::new(
        Arc::clone(&business_repo),
        Arc::clone(&category_repo),
        Arc::clone(&inquiry_repo),
        Arc::clone(&product_repo),
    );
    let product_usecase = ProductUseCase::new(Arc::clone(&product_repo));
    let admin_usecase   = AdminUseCase::new(
        Arc::clone(&business_repo),
        Arc::clone(&user_repo),
        Arc::clone(&category_repo),
        Arc::clone(&inquiry_repo),
        Arc::clone(&review_repo),
    );
    let auth_usecase    = AuthUseCase::new(
        Arc::clone(&user_repo),
        Arc::clone(&business_repo),
        EmailService::new(),
    );
    let provider_dashboard_usecase = ProviderDashboardUseCase::new(
        Arc::clone(&business_repo),
        Arc::clone(&product_repo),
    );
    let role_usecase  = RoleUseCase::new(Arc::clone(&role_repo));
    let setup_usecase = SetupUseCase::new(
        Arc::clone(&business_repo),
        Arc::clone(&category_repo),
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
            agri_connect:       agri_connect_usecase,
            product:            product_usecase,
            admin:              admin_usecase,
            auth:               auth_usecase,
            provider_dashboard: provider_dashboard_usecase,
            role:               role_usecase,
            setup:              setup_usecase,
            stats:              stats_usecase,
        },
    };

    // ── Router ────────────────────────────────────────────────────────────────
    let router = AppRouting::build(app_state);

    let listener = tokio::net::TcpListener::bind(crate::constants::SERVER_BIND_ADDR)
        .await
        .expect("Failed to bind server address");

    info!("Agri-Connect backend listening on http://{}", crate::constants::SERVER_BIND_ADDR);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .unwrap();
}
