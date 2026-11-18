use std::sync::Arc;

use application::ports::RateLimiter;
use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderValue},
    middleware,
    response::IntoResponse,
    routing::{delete, get, patch, post, put},
    Extension, Router,
};
use domain::{Error, ErrorCode};
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    trace::{DefaultOnResponse, TraceLayer},
};

use crate::{
    app_state::AppState,
    constants::{MULTIPART_OVERHEAD_BYTES, REQUEST_ID_MAX_LEN},
    error_response::render_errors,
    handlers::{
        admin_handler::AdminHandler,
        auth_handler::AuthHandler,
        coded_handler::{AlertKindHandler, SectorHandler},
        commodity_handler::CommodityHandler,
        explore_handler::ExploreHandler,
        health_handler::HealthHandler,
        job_handler::JobHandler,
        media_handler::MediaHandler,
        partner_handler::PartnerHandler,
        permission_handler::PermissionHandler,
        price_handler::PriceHandler,
        products_handler::ProductsHandler,
        provider_dashboard_handler::ProviderDashboardHandler,
        role_handler::RoleHandler,
        settings_handler::SettingsHandler,
        setup_handler::SetupHandler,
        stats_handler::StatsHandler,
        zalo_handler::ZaloHandler,
    },
    middleware::{
        client_ip, language, origin,
        rate_limit::{self, Limiters, Scope},
        timeout,
    },
    pages,
    routes::{self, FILE_PATH, FILE_THUMB_PATH},
    session, views,
};

/// Settings read from the environment by `startup.rs`.
#[derive(Clone)]
pub struct RouterConfig {
    /// Browser origins allowed to call the API cross-origin. Empty blocks all.
    pub cors_origins: Vec<HeaderValue>,
    /// Send HSTS. Only meaningful when TLS terminates in front of us; sent over
    /// plain HTTP it would lock browsers out of a site that has no TLS.
    pub https: bool,
    /// Reverse proxies in front of us whose X-Forwarded-For entry is trusted.
    /// 0: use the TCP peer. Too low behind Caddy: every user shares one rate
    /// limit. Too high: clients can forge their address.
    pub trusted_proxy_hops: usize,
    /// Origins whose non-GET requests are accepted (APP_URL + CORS_ORIGINS).
    /// Missing ours: every form on the site is refused as cross-site.
    pub allowed_origins: Vec<String>,
    /// Other sites allowed to call the API (CORS_ORIGINS): their non-GET
    /// requests are accepted only without a session cookie.
    pub api_origins: Vec<String>,
    pub limiters: Limiters,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            cors_origins: Vec::new(),
            https: false,
            trusted_proxy_hops: 0,
            allowed_origins: Vec::new(),
            api_origins: Vec::new(),
            limiters: Limiters::in_process(),
        }
    }
}

pub struct AppRouting;

impl AppRouting {
    pub fn build(state: AppState, config: RouterConfig) -> Router {
        // The cookies' names and attributes follow HTTPS_ENABLED; every
        // reader of the session cookie takes them from the state.
        let cookies = session::CookiePolicy {
            secure: config.https,
        };
        let state = AppState { cookies, ..state };
        let error_renderer = middleware::from_fn_with_state(state.clone(), render_errors);
        let hops = config.trusted_proxy_hops;
        // Rate limits: one budget per router area, keyed on the client address.
        let limiter = |l: &Arc<dyn RateLimiter>, scope: Scope| {
            let l = l.clone();
            middleware::from_fn(move |req, next| rate_limit::enforce(l.clone(), scope, req, next))
        };
        let origins = Arc::new(origin::Origins {
            own: config.allowed_origins.clone(),
            api: config.api_origins.clone(),
            session_cookies: [cookies.session_name(), cookies.refresh_name()],
        });

        let cors = if config.cors_origins.is_empty() {
            // No origins configured — block all cross-origin requests.
            CorsLayer::new()
        } else {
            CorsLayer::new()
                .allow_origin(config.cors_origins)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::PATCH,
                    axum::http::Method::DELETE,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                ])
        };

        // ── GUEST (public) routes ────────────────────────────────────────────
        // Contact form is rate-limited separately to prevent spam.
        let public_routes = Router::new()
            .route("/", get(pages::home::show))
            .route("/supplies", get(pages::products::list))
            .route(
                "/supplies/category/{id}/{slug}",
                get(pages::products::category),
            )
            .route("/supplies/{id}", get(pages::product::by_id))
            .route("/supplies/{id}/{slug}", get(pages::product::show))
            // Supplies were "products": the former addresses move for good (a
            // supply's or a category's page sends any spelling to its own).
            .route("/products", get(pages::moved::supplies))
            .route(
                "/products/category/{id}/{slug}",
                get(pages::products::category),
            )
            .route("/products/{id}", get(pages::product::by_id))
            .route("/products/{id}/{slug}", get(pages::product::show))
            .route("/services", get(pages::services::list))
            .route("/prices", get(pages::prices::show))
            .route("/jobs/new", get(pages::jobs::new))
            .route(
                "/services/category/{id}/{slug}",
                get(pages::services::category),
            )
            .route("/services/{id}", get(pages::service::by_id))
            .route("/services/{id}/{slug}", get(pages::service::show))
            .route("/providers", get(pages::directory::list))
            .route("/providers/{id}", get(pages::provider::by_id))
            .route("/providers/{id}/{slug}", get(pages::provider::show))
            .route(routes::HEALTH_LIVE_PATH, get(HealthHandler::live))
            .route(routes::HEALTH_READY_PATH, get(HealthHandler::ready))
            .route(
                "/api/explore/providers/{id}",
                get(ExploreHandler::get_provider_by_id),
            )
            .route(
                "/api/explore/providers/{id}/reviews",
                get(ExploreHandler::list_reviews),
            )
            .route(
                "/api/explore/services/{id}",
                get(ExploreHandler::get_service_by_id),
            )
            // /featured replaces the old /explore verb
            .route(
                "/api/explore/featured",
                get(ExploreHandler::get_explore_data),
            )
            .route("/api/explore/alerts", get(ExploreHandler::list_alerts))
            .route(
                "/api/explore/categories",
                get(ExploreHandler::get_categories),
            )
            .route("/api/explore/sectors", get(SectorHandler::shown))
            .route("/api/explore/alert-kinds", get(AlertKindHandler::shown))
            .route("/api/explore/commodities", get(CommodityHandler::shown))
            .route("/api/explore/locations", get(ExploreHandler::get_locations))
            .route("/api/explore/prices", get(PriceHandler::board))
            .route(
                "/api/explore/products/{id}",
                get(ProductsHandler::get_product_by_id),
            )
            // Uploaded images, same-origin so the page CSP can stay img-src 'self'.
            .route(FILE_PATH, get(MediaHandler::file))
            .route(FILE_THUMB_PATH, get(MediaHandler::thumb));

        // ── SEARCH routes (public reads with their own budget) ───────────────
        // Each is a text or geo query over the listings, and the search pages
        // also record what was searched.
        let search_routes = Router::new()
            .route("/explore", get(pages::explore::show))
            .route("/search", get(pages::search::show))
            // GET /api/explore/providers — geo-search when lat+lng present, browse otherwise
            .route(
                "/api/explore/providers",
                get(ExploreHandler::list_providers),
            )
            // GET /api/explore/services — near me when lat+lng present, browse otherwise
            .route("/api/explore/services", get(ExploreHandler::list_services))
            .route(
                "/api/explore/products",
                get(ProductsHandler::browse_products),
            )
            .layer(limiter(&config.limiters.public_search, Scope::AllRequests));

        let contact_routes = Router::new()
            .route(
                "/api/explore/contact",
                post(ExploreHandler::contact_request),
            )
            // The page form: same rules and budget as the JSON endpoint.
            .route("/contact", post(pages::provider::submit_contact))
            .route("/api/explore/reviews", post(ExploreHandler::submit_review))
            .route("/reviews", post(pages::provider::submit_review))
            .route("/api/explore/jobs", post(JobHandler::post))
            // The Zalo Mini App's token conversions.
            .route("/api/zalo/location", post(ZaloHandler::location))
            .route("/api/zalo/phone", post(ZaloHandler::phone))
            // The job form: same rules and budget as the JSON endpoint.
            .route("/jobs", post(pages::jobs::create))
            .route("/language", post(pages::language::switch))
            .route("/sector", post(pages::sector::switch))
            // Call-button taps: the click-to-call count, same budget.
            .route("/api/explore/calls", post(ExploreHandler::record_call))
            // Logging out is a write; under the same budget as the contact form.
            .route("/logout", post(session::logout))
            .route("/admin/logout", post(session::admin_logout))
            .layer(limiter(&config.limiters.public_write, Scope::AllRequests));

        // ── UPLOAD routes (own budget; body capped at UPLOAD_MAX_BYTES) ───────
        // Every request counts: a rejected upload was still read and maybe decoded.
        let upload_routes = Router::new()
            .route(
                "/api/provider/products/{id}/image",
                put(MediaHandler::put_product_image),
            )
            // The provider's supply forms carry the photo.
            .route(
                "/provider/supplies/new",
                post(pages::dashboard::create_product),
            )
            .route(
                "/provider/supplies/{id}/edit",
                post(pages::dashboard::update_product),
            )
            .route(
                "/api/provider/services/{id}/image",
                put(MediaHandler::put_service_image),
            )
            // The service forms carry an optional photo.
            .route(
                "/provider/services/new",
                post(pages::own_services::create_service),
            )
            .route(
                "/provider/services/{id}/edit",
                post(pages::own_services::update_service),
            )
            .layer(DefaultBodyLimit::max(
                state.usecases.media.max_upload_bytes() + MULTIPART_OVERHEAD_BYTES,
            ))
            .layer(limiter(&config.limiters.upload, Scope::AllRequests));

        // ── AUTH routes (no token required, rate-limited) ────────────────────
        let auth_routes = Router::new()
            // /tokens replaces the old /login verb
            .route("/api/auth/tokens", post(AuthHandler::login))
            // POST /api/users replaces the old /auth/register verb
            .route("/api/users", post(AuthHandler::register))
            .route(
                "/api/auth/password-resets",
                post(AuthHandler::forgot_password),
            )
            .route(
                "/api/auth/password-resets/{token}",
                patch(AuthHandler::reset_password),
            )
            // /setup replaces the old /initialize verb
            .route("/api/admin/setup", post(AuthHandler::create_first_admin))
            .layer(limiter(&config.limiters.auth, Scope::AllRequests));

        // ── AUTH pages: the same budget as the auth API, on the writes only ──
        // (reading the login page is free; trying passwords is not).
        let auth_pages = Router::new()
            .route(
                "/login",
                get(pages::auth::login_page).post(pages::auth::login),
            )
            .route(
                "/register",
                get(pages::auth::register_page).post(pages::auth::register),
            )
            .route(
                "/forgot-password",
                get(pages::auth::forgot_page).post(pages::auth::forgot),
            )
            .route(
                "/reset-password",
                get(pages::auth::reset_page).post(pages::auth::reset),
            )
            .route(
                "/verify-email",
                get(pages::auth::verify_email_page).post(pages::auth::verify_email),
            )
            .route(
                "/setup",
                get(pages::setup::setup_page).post(pages::setup::setup),
            )
            .layer(limiter(&config.limiters.auth, Scope::WritesOnly));

        // The demo data page: an admin action, on the signed-in budget.
        let member_pages = Router::new()
            .route(
                "/setup/seed",
                get(pages::setup::seed).post(pages::setup::seed_action),
            )
            .route("/account", get(pages::account::show))
            .route("/admin", get(pages::admin::dashboard::index))
            .route("/admin/dashboard", get(pages::admin::dashboard::show))
            .route("/admin/roles", get(pages::admin::roles::list))
            .route("/admin/roles/{id}", get(pages::admin::roles::show))
            .route(
                "/admin/roles/{id}/permissions",
                post(pages::admin::roles::save),
            )
            .route("/admin/permissions", get(pages::admin::roles::matrix))
            .route("/admin/users", get(pages::admin::users::list))
            .route("/admin/users/{id}", get(pages::admin::users::show))
            .route(
                "/admin/users/{id}/role",
                post(pages::admin::users::change_role),
            )
            .route(
                "/admin/users/{id}/status",
                post(pages::admin::users::change_status),
            )
            .route(
                "/admin/categories",
                get(pages::admin::categories::list).post(pages::admin::categories::create),
            )
            .route(
                "/admin/categories/{id}",
                get(pages::admin::categories::show).post(pages::admin::categories::update),
            )
            .route(
                "/admin/categories/{id}/status",
                post(pages::admin::categories::change_status),
            )
            .route(
                "/admin/prices",
                get(pages::admin::prices::list).post(pages::admin::prices::create),
            )
            .route(
                "/admin/prices/{id}/delete",
                post(pages::admin::prices::delete),
            )
            .route(
                "/admin/alert-kinds",
                get(pages::admin::alert_kinds::list).post(pages::admin::alert_kinds::create),
            )
            .route(
                "/admin/alert-kinds/{id}",
                get(pages::admin::alert_kinds::show).post(pages::admin::alert_kinds::update),
            )
            .route(
                "/admin/alert-kinds/{id}/status",
                post(pages::admin::alert_kinds::change_status),
            )
            .route(
                "/admin/sectors",
                get(pages::admin::sectors::list).post(pages::admin::sectors::create),
            )
            .route(
                "/admin/sectors/{id}",
                get(pages::admin::sectors::show).post(pages::admin::sectors::update),
            )
            .route(
                "/admin/sectors/{id}/status",
                post(pages::admin::sectors::change_status),
            )
            .route(
                "/admin/commodities",
                get(pages::admin::commodities::list).post(pages::admin::commodities::create),
            )
            .route(
                "/admin/commodities/{id}",
                get(pages::admin::commodities::show).post(pages::admin::commodities::update),
            )
            .route(
                "/admin/commodities/{id}/status",
                post(pages::admin::commodities::change_status),
            )
            .route(
                "/admin/crops",
                get(pages::admin::crops::list).post(pages::admin::crops::create),
            )
            .route(
                "/admin/crops/{id}",
                get(pages::admin::crops::show).post(pages::admin::crops::update),
            )
            .route(
                "/admin/crops/{id}/status",
                post(pages::admin::crops::change_status),
            )
            .route(
                "/admin/alerts",
                get(pages::admin::alerts::list).post(pages::admin::alerts::create),
            )
            .route(
                "/admin/alerts/{id}",
                get(pages::admin::alerts::show).post(pages::admin::alerts::update),
            )
            .route(
                "/admin/alerts/{id}/status",
                post(pages::admin::alerts::change_status),
            )
            .route(
                "/admin/locations",
                get(pages::admin::locations::list).post(pages::admin::locations::create),
            )
            .route(
                "/admin/locations/{id}",
                get(pages::admin::locations::show).post(pages::admin::locations::update),
            )
            .route(
                "/admin/locations/{id}/status",
                post(pages::admin::locations::change_status),
            )
            .route(
                "/admin/locations/{id}/delete",
                post(pages::admin::locations::delete),
            )
            // Former addresses, kept for bookmarks and old emails.
            .route("/admin/explore", get(pages::moved::admin_providers))
            .route("/admin/explore/{id}", get(pages::moved::admin_provider))
            .route("/admin/products", get(pages::moved::admin_supplies))
            .route("/admin/products/{id}", get(pages::moved::admin_supply))
            .route(
                "/admin/providers",
                get(pages::admin::providers::list).post(pages::admin::providers::create),
            )
            .route(
                "/admin/providers/{id}",
                get(pages::admin::providers::show).post(pages::admin::providers::update),
            )
            .route(
                "/admin/providers/{id}/verification",
                post(pages::admin::providers::set_verification),
            )
            .route(
                "/admin/providers/{id}/account",
                post(pages::admin::providers::assign_account),
            )
            .route(
                "/admin/providers/{id}/status",
                post(pages::admin::providers::change_status),
            )
            .route("/admin/supplies", get(pages::admin::products::list))
            .route(
                "/admin/supplies/{id}",
                get(pages::admin::products::show).post(pages::admin::products::update),
            )
            .route(
                "/admin/supplies/{id}/status",
                post(pages::admin::products::change_status),
            )
            .route(
                "/admin/supplies/{id}/delete",
                post(pages::admin::products::delete),
            )
            .route(
                "/admin/services",
                get(pages::admin::services::list).post(pages::admin::services::create),
            )
            .route(
                "/admin/services/{id}",
                get(pages::admin::services::show).post(pages::admin::services::update),
            )
            .route(
                "/admin/services/{id}/status",
                post(pages::admin::services::change_status),
            )
            .route(
                "/admin/services/{id}/delete",
                post(pages::admin::services::delete),
            )
            .route("/admin/inquiries", get(pages::admin::inquiries::list))
            .route(
                "/admin/inquiries/{id}/status",
                post(pages::admin::inquiries::change_status),
            )
            .route("/admin/jobs", get(pages::admin::jobs::list))
            .route(
                "/admin/jobs/{id}/status",
                post(pages::admin::jobs::change_status),
            )
            .route(
                "/admin/settings",
                get(pages::admin::settings::show).post(pages::admin::settings::update),
            )
            .route("/admin/demand", get(pages::admin::demand::show))
            .route("/admin/demand.csv", get(pages::admin::demand::csv))
            .route(
                "/admin/partners",
                get(pages::admin::partners::list).post(pages::admin::partners::create),
            )
            .route(
                "/admin/partners/{id}",
                get(pages::admin::partners::show).post(pages::admin::partners::update),
            )
            .route(
                "/admin/partners/{id}/status",
                post(pages::admin::partners::change_status),
            )
            .route(
                "/admin/partners/{id}/account",
                post(pages::admin::partners::assign_account),
            )
            .route(
                "/admin/partners/{id}/contracts",
                post(pages::admin::partners::create_contract),
            )
            .route(
                "/admin/partners/{id}/contracts/{cid}",
                post(pages::admin::partners::update_contract),
            )
            .route(
                "/admin/partners/{id}/contracts/{cid}/delete",
                post(pages::admin::partners::delete_contract),
            )
            .route(
                "/admin/partners/{id}/contracts/{cid}/sponsored",
                post(pages::admin::partners::add_sponsored),
            )
            .route(
                "/admin/partners/{id}/contracts/{cid}/sponsored/{bid}/delete",
                post(pages::admin::partners::remove_sponsored),
            )
            .route("/partner", get(pages::partner::show))
            .route("/partner/report.csv", get(pages::partner::csv))
            .route("/admin/reviews", get(pages::admin::reviews::list))
            .route(
                "/admin/reviews/{id}/visibility",
                post(pages::admin::reviews::change_visibility),
            )
            .route("/provider/dashboard", get(pages::dashboard::show))
            .route("/provider/profile", post(pages::dashboard::update_profile))
            .route("/provider/supplies/new", get(pages::dashboard::new_product))
            .route(
                "/provider/supplies/{id}/edit",
                get(pages::dashboard::edit_product),
            )
            .route("/provider/products/new", get(pages::moved::own_supply_new))
            .route(
                "/provider/products/{id}/edit",
                get(pages::moved::own_supply_edit),
            )
            .route(
                "/provider/supplies/{id}/delete",
                post(pages::dashboard::delete_product),
            )
            .route(
                "/provider/services/new",
                get(pages::own_services::new_service),
            )
            .route(
                "/provider/services/{id}/edit",
                get(pages::own_services::edit_service),
            )
            .route(
                "/provider/services/{id}/delete",
                post(pages::own_services::delete_service),
            )
            .route("/provider/reviews", get(pages::own_reviews::list))
            .route(
                "/provider/reviews/{id}/reply",
                post(pages::own_reviews::reply),
            )
            .route("/provider/jobs", get(pages::own_jobs::list))
            .route("/provider/inquiries", get(pages::inbox::list))
            .route(
                "/provider/inquiries/{id}/status",
                post(pages::inbox::change_status),
            )
            .route("/account/password", post(pages::account::change_password))
            .route(
                "/account/email-verification",
                post(pages::account::send_email_verification),
            );

        // ── USER routes (authenticated, no rate limit) ───────────────────────
        let user_routes = Router::new()
            .route("/api/users/me", get(AuthHandler::me))
            .route(
                "/api/users/me/password",
                patch(AuthHandler::change_password),
            );

        // ── ADMIN routes ─────────────────────────────────────────────────────
        let admin_routes = Router::new()
            .route("/api/setup/status", get(SetupHandler::get_status))
            .route(
                "/api/setup/seed",
                post(SetupHandler::seed).delete(SetupHandler::remove_seed),
            )
            .route("/api/admin/stats", get(StatsHandler::get_stats))
            // Providers
            .route(
                "/api/admin/providers",
                get(AdminHandler::list_providers).post(AdminHandler::create_provider),
            )
            .route(
                "/api/admin/providers/{id}",
                get(AdminHandler::get_provider_detail).put(AdminHandler::update_provider),
            )
            .route(
                "/api/admin/providers/{id}/verification",
                patch(AdminHandler::set_verification),
            )
            .route(
                "/api/admin/providers/{id}/status",
                patch(AdminHandler::toggle_provider_active),
            )
            .route(
                "/api/admin/providers/{id}/users",
                post(AdminHandler::assign_provider_user),
            )
            // Roles (read-only — roles are managed via migrations, not runtime API)
            .route("/api/admin/roles", get(RoleHandler::list))
            .route("/api/admin/roles/{id}", get(RoleHandler::get_by_id))
            // Users
            .route("/api/admin/users", get(AdminHandler::list_users))
            .route("/api/admin/users/{id}", get(AdminHandler::get_user_detail))
            .route(
                "/api/admin/users/{id}/role",
                patch(AdminHandler::update_user_role),
            )
            .route(
                "/api/admin/users/{id}/status",
                patch(AdminHandler::toggle_user_active),
            )
            // Categories
            .route("/api/admin/categories", get(AdminHandler::list_categories))
            .route("/api/admin/categories", post(AdminHandler::create_category))
            .route(
                "/api/admin/categories/slug",
                get(AdminHandler::suggest_category_slug),
            )
            .route(
                "/api/admin/categories/{id}",
                get(AdminHandler::get_category_detail),
            )
            .route(
                "/api/admin/categories/{id}",
                put(AdminHandler::update_category),
            )
            .route(
                "/api/admin/categories/{id}/status",
                patch(AdminHandler::toggle_category_active),
            )
            // Inquiries
            .route("/api/admin/inquiries", get(AdminHandler::list_inquiries))
            .route(
                "/api/admin/inquiries/{id}/status",
                patch(AdminHandler::update_inquiry_status),
            )
            // Reviews
            .route(
                "/api/admin/reports/demand",
                get(StatsHandler::demand_report),
            )
            .route(
                "/api/admin/partners",
                get(PartnerHandler::list).post(PartnerHandler::create),
            )
            .route("/api/admin/partners/kpis", get(PartnerHandler::kpis))
            .route(
                "/api/admin/partners/{id}",
                get(PartnerHandler::get).put(PartnerHandler::update),
            )
            .route(
                "/api/admin/partners/{id}/status",
                patch(PartnerHandler::set_status),
            )
            .route(
                "/api/admin/partners/{id}/account",
                post(PartnerHandler::assign_account),
            )
            .route(
                "/api/admin/partners/{id}/contracts",
                post(PartnerHandler::create_contract),
            )
            .route(
                "/api/admin/partners/{id}/contracts/{cid}",
                delete(PartnerHandler::delete_contract).put(PartnerHandler::update_contract),
            )
            .route(
                "/api/admin/partners/{id}/contracts/{cid}/sponsored",
                post(PartnerHandler::add_sponsored),
            )
            .route(
                "/api/admin/partners/{id}/contracts/{cid}/sponsored/{bid}",
                delete(PartnerHandler::remove_sponsored),
            )
            .route("/api/partner/report", get(PartnerHandler::own_report))
            .route("/api/admin/reviews", get(AdminHandler::list_reviews))
            .route(
                "/api/admin/reviews/{id}/visibility",
                patch(AdminHandler::toggle_review_visible),
            )
            // Permissions (read-only; grants belong to roles)
            .route("/api/admin/permissions", get(PermissionHandler::list_all))
            .route(
                "/api/admin/settings",
                get(SettingsHandler::list).patch(SettingsHandler::update),
            )
            .route(
                "/api/admin/users/{id}/permissions",
                get(PermissionHandler::list_by_user),
            )
            // Job posts
            .route("/api/provider/jobs", get(JobHandler::list_own))
            .route("/api/admin/jobs", get(JobHandler::list_admin))
            .route("/api/admin/jobs/{id}/status", patch(JobHandler::set_status))
            // Alert kinds
            .route(
                "/api/admin/alert-kinds",
                get(AlertKindHandler::list).post(AlertKindHandler::create),
            )
            .route(
                "/api/admin/alert-kinds/{id}",
                get(AlertKindHandler::get).put(AlertKindHandler::update),
            )
            .route(
                "/api/admin/alert-kinds/{id}/status",
                patch(AlertKindHandler::set_status),
            )
            // Sectors
            .route(
                "/api/admin/sectors",
                get(SectorHandler::list).post(SectorHandler::create),
            )
            .route(
                "/api/admin/sectors/{id}",
                get(SectorHandler::get).put(SectorHandler::update),
            )
            .route(
                "/api/admin/sectors/{id}/status",
                patch(SectorHandler::set_status),
            )
            // Crop prices
            .route(
                "/api/admin/commodities",
                get(CommodityHandler::list).post(CommodityHandler::create),
            )
            .route(
                "/api/admin/commodities/{id}",
                get(CommodityHandler::get).put(CommodityHandler::update),
            )
            .route(
                "/api/admin/commodities/{id}/status",
                patch(CommodityHandler::set_status),
            )
            .route(
                "/api/admin/crops",
                get(PriceHandler::list_crops).post(PriceHandler::create_crop),
            )
            .route(
                "/api/admin/crops/{id}",
                get(PriceHandler::get_crop).put(PriceHandler::update_crop),
            )
            .route(
                "/api/admin/crops/{id}/status",
                patch(PriceHandler::set_crop_status),
            )
            .route(
                "/api/admin/prices",
                get(PriceHandler::list_prices).post(PriceHandler::record_price),
            )
            .route("/api/admin/prices/{id}", delete(PriceHandler::delete_price))
            // Locations
            .route("/api/admin/locations", get(AdminHandler::list_locations))
            .route("/api/admin/locations", post(AdminHandler::create_location))
            .route("/api/admin/locations/{id}", get(AdminHandler::get_location))
            .route(
                "/api/admin/locations/{id}",
                put(AdminHandler::update_location),
            )
            .route(
                "/api/admin/locations/{id}/status",
                patch(AdminHandler::toggle_location_active),
            )
            .route(
                "/api/admin/locations/{id}",
                delete(AdminHandler::delete_location),
            )
            // Products
            .route("/api/admin/products", get(AdminHandler::list_products))
            .route(
                "/api/admin/products/{id}",
                get(AdminHandler::get_product).put(AdminHandler::update_product),
            )
            .route(
                "/api/admin/products/{id}/active",
                patch(AdminHandler::toggle_product_active),
            )
            .route(
                "/api/admin/products/{id}",
                delete(AdminHandler::delete_product),
            )
            // Services
            .route("/api/admin/services", get(AdminHandler::list_services))
            .route("/api/admin/services", post(AdminHandler::create_service))
            .route("/api/admin/services/{id}", get(AdminHandler::get_service))
            .route(
                "/api/admin/services/{id}",
                put(AdminHandler::update_service),
            )
            .route(
                "/api/admin/services/{id}/active",
                patch(AdminHandler::toggle_service_active),
            )
            .route(
                "/api/admin/services/{id}",
                delete(AdminHandler::delete_service),
            );

        // ── PROVIDER routes ──────────────────────────────────────────────────
        let provider_routes = Router::new()
            .route(
                "/api/provider/profile",
                get(ProviderDashboardHandler::get_profile),
            )
            .route(
                "/api/provider/profile",
                put(ProviderDashboardHandler::update_profile),
            )
            .route(
                "/api/provider/products",
                get(ProviderDashboardHandler::list_products),
            )
            .route(
                "/api/provider/products",
                post(ProviderDashboardHandler::create_product),
            )
            .route(
                "/api/provider/products/{id}",
                put(ProviderDashboardHandler::update_product),
            )
            .route(
                "/api/provider/products/{id}",
                delete(ProviderDashboardHandler::delete_product),
            )
            .route(
                "/api/provider/services",
                get(ProviderDashboardHandler::list_services),
            )
            .route(
                "/api/provider/services",
                post(ProviderDashboardHandler::create_service),
            )
            .route(
                "/api/provider/services/{id}",
                put(ProviderDashboardHandler::update_service),
            )
            .route(
                "/api/provider/services/{id}",
                delete(ProviderDashboardHandler::delete_service),
            )
            .route(
                "/api/provider/reviews",
                get(ProviderDashboardHandler::list_reviews),
            )
            .route(
                "/api/provider/reviews/{id}/reply",
                put(ProviderDashboardHandler::reply_review),
            );

        let mut router = Router::new()
            .merge(public_routes)
            .merge(search_routes)
            .merge(contact_routes)
            .merge(auth_routes)
            .merge(auth_pages)
            .merge(member_pages.layer(limiter(&config.limiters.member_write, Scope::WritesOnly)))
            .merge(user_routes.layer(limiter(&config.limiters.member_write, Scope::WritesOnly)))
            .merge(admin_routes.layer(limiter(&config.limiters.member_write, Scope::WritesOnly)))
            .merge(provider_routes.layer(limiter(&config.limiters.member_write, Scope::WritesOnly)))
            .route(routes::STATIC_ROUTE, get(views::static_file))
            .route(views::FAVICON_PATH, get(views::favicon))
            // Every route above gives up after REQUEST_TIMEOUT_SECS; the
            // uploads, merged after it, may take as long as the signal needs.
            .layer(middleware::from_fn(timeout::enforce))
            .merge(upload_routes)
            .fallback(fallback)
            .layer(cors)
            .layer(SetResponseHeaderLayer::if_not_present(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(
                header::X_FRAME_OPTIONS,
                HeaderValue::from_static("DENY"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(
                header::REFERRER_POLICY,
                HeaderValue::from_static("strict-origin-when-cross-origin"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static(
                    "default-src 'none'; \
                     script-src 'none'; \
                     frame-ancestors 'none'",
                ),
            ));

        if config.https {
            router = router.layer(SetResponseHeaderLayer::if_not_present(
                header::STRICT_TRANSPORT_SECURITY,
                HeaderValue::from_static("max-age=31536000; includeSubDomains"),
            ));
        }

        let request_id = header::HeaderName::from_static(REQUEST_ID_HEADER);
        router
            // Errors from every layer above (rate limiter, extractors, fallback)
            // get a rendered body; it sits inside the request span so its log
            // lines carry the request id.
            // Renews an expired session before any handler reads the cookie. Inside the Origin check: a refused
            // cross-site request never rotates a token.
            .layer(middleware::from_fn_with_state(
                state.clone(),
                session::renew,
            ))
            .layer(Extension(cookies))
            // Inside the renderer so a refused cross-site request gets a coded body.
            .layer(middleware::from_fn(move |req, next| {
                origin::enforce(origins.clone(), req, next)
            }))
            .layer(error_renderer)
            // Outermost of the page layers: every page and error body is worded
            // in the visitor's language.
            .layer(middleware::from_fn(language::scope_request))
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(request_span)
                    .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
            )
            .layer(middleware::from_fn(move |req, next| {
                client_ip::resolve(hops, req, next)
            }))
            .layer(PropagateRequestIdLayer::new(request_id.clone()))
            .layer(SetRequestIdLayer::new(request_id, MakeRequestUuid))
            // Outermost: a client's own id is kept only when it looks like
            // one; anything else is dropped and a fresh id made above.
            .layer(middleware::from_fn(plain_request_id))
            .with_state(state)
    }
}

/// Echoed on every response, so a user's bug report can be matched to the log
/// lines of that exact request. A client-supplied value is kept when it is a
/// plain id (`plain_request_id`).
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Whether a client's request id may go into the log lines and the error
/// page: short, and only letters, digits, `-` and `_`.
pub fn is_plain_request_id(value: &[u8]) -> bool {
    !value.is_empty()
        && value.len() <= REQUEST_ID_MAX_LEN
        && value
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

/// Drops a client's `x-request-id` that is not a plain id, so the server
/// makes its own: a long or odd value never reaches the logs.
async fn plain_request_id(
    mut req: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let plain = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .is_none_or(|v| is_plain_request_id(v.as_bytes()));
    if !plain {
        req.headers_mut().remove(REQUEST_ID_HEADER);
    }
    next.run(req).await
}

fn request_span(req: &axum::http::Request<axum::body::Body>) -> tracing::Span {
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    tracing::info_span!(
        "request",
        request_id,
        method = %req.method(),
        path = req.uri().path(),
    )
}

/// Anything no route matched: a coded 404 (a page for a browser, JSON for
/// the API).
async fn fallback() -> axum::response::Response {
    Error::new(ErrorCode::RouteNotFound).into_response()
}
