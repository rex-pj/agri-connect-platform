use axum::{
    http::{header, HeaderValue},
    middleware,
    routing::{delete, get, patch, post, put},
    Extension, Router,
};
use tower_http::{
    cors::CorsLayer,
    set_header::SetResponseHeaderLayer,
};

use crate::{
    app_state::AppState,
    middleware::rate_limit,
    handlers::{
        admin_handler::AdminHandler,
        agri_connect_handler::AgriConnectHandler,
        auth_handler::AuthHandler,
        health_handler::HealthHandler,
        products_handler::ProductsHandler,
        provider_dashboard_handler::ProviderDashboardHandler,
        role_handler::RoleHandler,
        setup_handler::SetupHandler,
        stats_handler::StatsHandler,
    },
};

pub struct AppRouting;

impl AppRouting {
    pub fn build(state: AppState) -> Router {
        // ── Rate limiters (per-IP, in-memory) ────────────────────────────────
        let auth_limiter         = rate_limit::new_auth_limiter();
        let public_write_limiter = rate_limit::new_public_write_limiter();

        let allowed_origins: Vec<HeaderValue> = std::env::var("CORS_ALLOW_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();

        let cors = if allowed_origins.is_empty() {
            // No origins configured — block all cross-origin requests.
            CorsLayer::new()
        } else {
            CorsLayer::new()
                .allow_origin(allowed_origins)
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
            .route("/health",                                get(HealthHandler::health))
            .route("/api/agri-connect/providers",           get(AgriConnectHandler::search_providers))
            .route("/api/agri-connect/providers/browse",    get(AgriConnectHandler::browse_providers))
            .route("/api/agri-connect/providers/{id}",      get(AgriConnectHandler::get_provider_by_id))
            .route("/api/agri-connect/explore",             get(AgriConnectHandler::get_explore_data))
            .route("/api/agri-connect/categories",          get(AgriConnectHandler::get_categories))
            .route("/api/agri-connect/products/browse",     get(ProductsHandler::browse_products))
            .route("/api/agri-connect/products/{id}",       get(ProductsHandler::get_product_by_id));

        let contact_routes = Router::new()
            .route("/api/agri-connect/contact", post(AgriConnectHandler::contact_request))
            .layer(middleware::from_fn(rate_limit::enforce))
            .layer(Extension(public_write_limiter));

        // ── AUTH routes (no token required, rate-limited) ────────────────────
        let auth_routes = Router::new()
            .route("/api/auth/login",                       post(AuthHandler::login))
            .route("/api/auth/register",                    post(AuthHandler::register))
            .route("/api/auth/password-resets",             post(AuthHandler::forgot_password))
            .route("/api/auth/password-resets/{token}",     patch(AuthHandler::reset_password))
            .route("/api/admin/initialize",                 post(AuthHandler::create_first_admin))
            .layer(middleware::from_fn(rate_limit::enforce))
            .layer(Extension(auth_limiter));

        // ── USER routes (authenticated, no rate limit) ───────────────────────
        let user_routes = Router::new()
            .route("/api/users/me",             get(AuthHandler::me))
            .route("/api/users/me/password",    patch(AuthHandler::change_password));

        // ── ADMIN routes ─────────────────────────────────────────────────────
        let admin_routes = Router::new()
            .route("/api/setup/status",                             get(SetupHandler::get_status))
            .route("/api/setup/data",                               post(SetupHandler::seed))
            .route("/api/admin/stats",                              get(StatsHandler::get_stats))
            // Providers
            .route("/api/admin/providers",                          get(AdminHandler::list_providers))
            .route("/api/admin/providers/{id}",                     get(AdminHandler::get_provider_detail))
            .route("/api/admin/providers/{id}/verification",        patch(AdminHandler::toggle_verify))
            .route("/api/admin/providers/{id}/users",               post(AdminHandler::assign_provider_user))
            // Roles (read-only — roles are managed via migrations, not runtime API)
            .route("/api/admin/roles",                          get(RoleHandler::list))
            .route("/api/admin/roles/{id}",                     get(RoleHandler::get_by_id))
            // Users
            .route("/api/admin/users",                          get(AdminHandler::list_users))
            .route("/api/admin/users/{id}",                     get(AdminHandler::get_user_detail))
            .route("/api/admin/users/{id}/role",                patch(AdminHandler::update_user_role))
            .route("/api/admin/users/{id}/active",              patch(AdminHandler::toggle_user_active))
            // Categories
            .route("/api/admin/categories",                     get(AdminHandler::list_categories))
            .route("/api/admin/categories",                     post(AdminHandler::create_category))
            .route("/api/admin/categories/{id}",                get(AdminHandler::get_category_detail))
            .route("/api/admin/categories/{id}",                put(AdminHandler::update_category))
            .route("/api/admin/categories/{id}/active",         patch(AdminHandler::toggle_category_active))
            // Inquiries
            .route("/api/admin/inquiries",                      get(AdminHandler::list_inquiries))
            .route("/api/admin/inquiries/{id}/status",          patch(AdminHandler::update_inquiry_status))
            // Reviews
            .route("/api/admin/reviews",                        get(AdminHandler::list_reviews))
            .route("/api/admin/reviews/{id}/visible",           patch(AdminHandler::toggle_review_visible));

        // ── PROVIDER routes ──────────────────────────────────────────────────
        let provider_routes = Router::new()
            .route("/api/provider/profile",         get(ProviderDashboardHandler::get_profile))
            .route("/api/provider/profile",         put(ProviderDashboardHandler::update_profile))
            .route("/api/provider/products",        get(ProviderDashboardHandler::list_products))
            .route("/api/provider/products",        post(ProviderDashboardHandler::create_product))
            .route("/api/provider/products/{id}",   put(ProviderDashboardHandler::update_product))
            .route("/api/provider/products/{id}",   delete(ProviderDashboardHandler::delete_product));

        // HSTS is only meaningful over HTTPS — opt-in via HTTPS_ENABLED=true in production.
        let https_enabled = std::env::var("HTTPS_ENABLED").as_deref() == Ok("true");

        let mut router = Router::new()
            .merge(public_routes)
            .merge(contact_routes)
            .merge(auth_routes)
            .merge(user_routes)
            .merge(admin_routes)
            .merge(provider_routes)
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

        if https_enabled {
            router = router.layer(SetResponseHeaderLayer::if_not_present(
                header::STRICT_TRANSPORT_SECURITY,
                HeaderValue::from_static("max-age=31536000; includeSubDomains"),
            ));
        }

        router.with_state(state)
    }
}
