use std::sync::Arc;

use application::{
    ports::TokenService,
    usecases::{
        admin_usecase::AdminUseCase, auth_usecase::AuthUseCase, explore_usecase::ExploreUseCase,
        permission_usecase::PermissionUseCase, product_usecase::ProductUseCase,
        provider_dashboard_usecase::ProviderDashboardUseCase, setup_usecase::SetupUseCase,
        stats_usecase::StatsUseCase,
    },
};

use crate::locale::Locale;

pub struct UseCases {
    pub explore:            ExploreUseCase,
    pub product:            ProductUseCase,
    pub admin:              AdminUseCase,
    pub auth:               AuthUseCase,
    pub permission:         PermissionUseCase,
    pub provider_dashboard: ProviderDashboardUseCase,
    pub setup:              SetupUseCase,
    pub stats:              StatsUseCase,
}

/// Cloned into every request, so it holds only `Arc`s. `startup.rs` builds it.
#[derive(Clone)]
pub struct AppState {
    pub usecases: Arc<UseCases>,
    /// The web layer needs it to read the session token; nothing else of
    /// infrastructure is exposed here.
    pub token_service: Arc<dyn TokenService>,
    pub locale: Arc<Locale>,
}
