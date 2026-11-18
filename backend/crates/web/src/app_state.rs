use std::sync::Arc;

use application::{
    ports::TokenService,
    usecases::{
        admin_usecase::AdminUseCase, alert_usecase::AlertUseCase, auth_usecase::AuthUseCase,
        coded_usecase::CodedUseCase, commodity_usecase::CommodityUseCase,
        demand_usecase::DemandReportUseCase, explore_usecase::ExploreUseCase,
        health_usecase::HealthUseCase, job_usecase::JobUseCase, media_usecase::MediaUseCase,
        partner_usecase::PartnerUseCase, permission_usecase::PermissionUseCase,
        price_usecase::PriceUseCase, product_usecase::ProductUseCase,
        provider_dashboard_usecase::ProviderDashboardUseCase, review_usecase::ReviewUseCase,
        settings_usecase::SettingsUseCase, setup_usecase::SetupUseCase,
        stats_usecase::StatsUseCase, zalo_usecase::ZaloUseCase,
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
    pub alert: AlertUseCase,
    pub commodity: CommodityUseCase,
    /// The sectors the platform serves.
    pub sector: CodedUseCase,
    /// The kinds of alert.
    pub alert_kind: CodedUseCase,
    pub auth: AuthUseCase,
    pub demand: DemandReportUseCase,
    pub partner: PartnerUseCase,
    pub permission: PermissionUseCase,
    pub price: PriceUseCase,
    pub provider_dashboard: ProviderDashboardUseCase,
    pub review: ReviewUseCase,
    pub setup: SetupUseCase,
    /// The settings an admin changes without a deploy.
    pub settings: SettingsUseCase,
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
    /// The session cookies' names and attributes (`__Host-` behind TLS);
    /// set by the router from HTTPS_ENABLED.
    pub cookies: crate::session::CookiePolicy,
}

impl AppState {
    /// The words of the language this request is served in.
    pub fn locale(&self) -> &Locale {
        self.locales.current()
    }
}
