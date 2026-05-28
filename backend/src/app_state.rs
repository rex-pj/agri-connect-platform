use crate::application::usecases::{
    admin_usecase::AdminUseCase,
    explore_usecase::ExploreUseCase,
    auth_usecase::AuthUseCase,
    permission_usecase::PermissionUseCase,
    product_usecase::ProductUseCase,
    provider_dashboard_usecase::ProviderDashboardUseCase,
    setup_usecase::SetupUseCase,
    stats_usecase::StatsUseCase,
};

#[derive(Clone)]
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

#[derive(Clone)]
pub struct AppState {
    pub usecases: UseCases,
}
