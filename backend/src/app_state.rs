use crate::application::usecases::{
    admin_usecase::AdminUseCase,
    agri_connect_usecase::AgriConnectUseCase,
    auth_usecase::AuthUseCase,
    product_usecase::ProductUseCase,
    provider_dashboard_usecase::ProviderDashboardUseCase,
    role_usecase::RoleUseCase,
    setup_usecase::SetupUseCase,
    stats_usecase::StatsUseCase,
};

#[derive(Clone)]
pub struct UseCases {
    pub agri_connect:       AgriConnectUseCase,
    pub product:            ProductUseCase,
    pub admin:              AdminUseCase,
    pub auth:               AuthUseCase,
    pub provider_dashboard: ProviderDashboardUseCase,
    pub role:               RoleUseCase,
    pub setup:              SetupUseCase,
    pub stats:              StatsUseCase,
}

#[derive(Clone)]
pub struct AppState {
    pub usecases: UseCases,
}
