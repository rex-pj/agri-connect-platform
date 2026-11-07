use std::sync::Arc;

use application::{
    ports::TokenService,
    usecases::{
        admin_usecase::AdminUseCase, auth_usecase::AuthUseCase,
        demand_usecase::DemandReportUseCase, explore_usecase::ExploreUseCase,
        health_usecase::HealthUseCase, job_usecase::JobUseCase, media_usecase::MediaUseCase,
        partner_usecase::PartnerUseCase, permission_usecase::PermissionUseCase,
        price_usecase::PriceUseCase, product_usecase::ProductUseCase,
        provider_dashboard_usecase::ProviderDashboardUseCase, review_usecase::ReviewUseCase,
        setup_usecase::SetupUseCase, stats_usecase::StatsUseCase, zalo_usecase::ZaloUseCase,
    },
};

use crate::{
    locale::{Locale, Locales},
    views::Views,
};

pub struct UseCases {
    pub explore: ExploreUseCase,
    pub health: HealthUseCase,
    pub job: JobUseCase,
    pub media: MediaUseCase,
    pub product: ProductUseCase,
    pub admin: AdminUseCase,
    pub auth: AuthUseCase,
    pub demand: DemandReportUseCase,
    pub partner: PartnerUseCase,
    pub permission: PermissionUseCase,
    pub price: PriceUseCase,
    pub provider_dashboard: ProviderDashboardUseCase,
    pub review: ReviewUseCase,
    pub setup: SetupUseCase,
    pub stats: StatsUseCase,
    pub zalo: ZaloUseCase,
}

/// Cloned into every request, so it holds only `Arc`s. `startup.rs` builds it.
#[derive(Clone)]
pub struct AppState {
    pub usecases: Arc<UseCases>,
    /// The web layer needs it to read the session token; nothing else of
    /// infrastructure is exposed here.
    pub token_service: Arc<dyn TokenService>,
    /// Every language; `locale()` picks the request's.
    pub locales: Arc<Locales>,
    pub views: Arc<Views>,
}

impl AppState {
    /// The words of the language this request is served in.
    pub fn locale(&self) -> &Locale {
        self.locales.current()
    }
}
