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
    constants::MULTIPART_OVERHEAD_BYTES,
    error_response::render_errors,
    handlers::{
        admin_handler::AdminHandler, auth_handler::AuthHandler, explore_handler::ExploreHandler,
        health_handler::HealthHandler, media_handler::MediaHandler,
        permission_handler::PermissionHandler, products_handler::ProductsHandler,
        provider_dashboard_handler::ProviderDashboardHandler, role_handler::RoleHandler,
        setup_handler::SetupHandler, stats_handler::StatsHandler,
    },
    legacy::{is_rust_only, LegacyProxy},
    middleware::{
        client_ip, origin,
        rate_limit::{self, Limiters, Scope},
    },
    pages,
    routes::{FILE_PATH, FILE_THUMB_PATH},
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
    /// Where pages Rust does not serve yet are forwarded. `None`: they 404.
    pub legacy: Option<LegacyProxy>,
    /// Reverse proxies in front of us whose X-Forwarded-For entry is trusted.
    /// 0: use the TCP peer. Too low behind Caddy: every user shares one rate
    /// limit. Too high: clients can forge their address.
    pub trusted_proxy_hops: usize,
    /// Origins whose non-GET requests are accepted (APP_URL + CORS_ORIGINS).
    /// Missing ours: every form on the site is refused as cross-site.
    pub allowed_origins: Vec<String>,
    pub limiters: Limiters,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            cors_origins: Vec::new(),
            https: false,
            legacy: None,
            trusted_proxy_hops: 0,
            allowed_origins: Vec::new(),
            limiters: Limiters::in_process(),
        }
    }
}

pub struct AppRouting;

impl AppRouting {
    pub fn build(state: AppState, config: RouterConfig) -> Router {
        let error_renderer = middleware::from_fn_with_state(state.clone(), render_errors);
        let legacy = config.legacy.clone();
        let hops = config.trusted_proxy_hops;
        // Rate limits: one budget per router area, keyed on the client address.
        let limiter = |l: &Arc<dyn RateLimiter>, scope: Scope| {
            let l = l.clone();
            middleware::from_fn(move |req, next| rate_limit::enforce(l.clone(), scope, req, next))
        };
        let allowed_origins = Arc::new(config.allowed_origins.clone());

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
            .route("/products", get(pages::products::list))
            .route(
                "/products/category/{id}/{slug}",
                get(pages::products::category),
            )
            .route("/products/{id}", get(pages::product::by_id))
            .route("/products/{id}/{slug}", get(pages::product::show))
            .route("/explore", get(pages::explore::show))
            .route("/search", get(pages::search::show))
            .route("/services", get(pages::services::list))
            .route(
                "/services/category/{id}/{slug}",
                get(pages::services::category),
            )
            .route("/providers/{id}", get(pages::provider::by_id))
            .route("/providers/{id}/{slug}", get(pages::provider::show))
            .route("/health/live", get(HealthHandler::live))
            .route("/health/ready", get(HealthHandler::ready))
            // GET /api/explore/providers — geo-search when lat+lng present, browse otherwise
            .route(
                "/api/explore/providers",
                get(ExploreHandler::list_providers),
            )
            .route(
                "/api/explore/providers/{id}",
                get(ExploreHandler::get_provider_by_id),
            )
            // /featured replaces the old /explore verb
            .route(
                "/api/explore/featured",
                get(ExploreHandler::get_explore_data),
            )
            .route(
                "/api/explore/categories",
                get(ExploreHandler::get_categories),
            )
            .route("/api/explore/locations", get(ExploreHandler::get_locations))
            // GET /api/explore/products — replaces the old /products/browse verb path
            .route(
                "/api/explore/products",
                get(ProductsHandler::browse_products),
            )
            .route(
                "/api/explore/products/{id}",
                get(ProductsHandler::get_product_by_id),
            )
            // Uploaded images, same-origin so the page CSP can stay img-src 'self'.
            .route(FILE_PATH, get(MediaHandler::file))
            .route(FILE_THUMB_PATH, get(MediaHandler::thumb));

        let contact_routes = Router::new()
            .route(
                "/api/explore/contact",
                post(ExploreHandler::contact_request),
            )
            // The page form: same rules and budget as the JSON endpoint.
            .route("/contact", post(pages::provider::submit_contact))
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
            .route("/account/password", post(pages::account::change_password));

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
            .route("/api/admin/providers", get(AdminHandler::list_providers))
            .route(
                "/api/admin/providers/{id}",
                get(AdminHandler::get_provider_detail),
            )
            .route(
                "/api/admin/providers/{id}/verification",
                patch(AdminHandler::toggle_verify),
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
            .route("/api/admin/reviews", get(AdminHandler::list_reviews))
            .route(
                "/api/admin/reviews/{id}/visibility",
                patch(AdminHandler::toggle_review_visible),
            )
            // Permissions (read-only; grants belong to roles)
            .route("/api/admin/permissions", get(PermissionHandler::list_all))
            .route(
                "/api/admin/users/{id}/permissions",
                get(PermissionHandler::list_by_user),
            )
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
            .route("/api/admin/products/{id}", get(AdminHandler::get_product))
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
            );

        let mut router = Router::new()
            .merge(public_routes)
            .merge(contact_routes)
            .merge(upload_routes)
            .merge(auth_routes)
            .merge(auth_pages)
            .merge(member_pages.layer(limiter(&config.limiters.member_write, Scope::WritesOnly)))
            .merge(user_routes.layer(limiter(&config.limiters.member_write, Scope::WritesOnly)))
            .merge(admin_routes.layer(limiter(&config.limiters.member_write, Scope::WritesOnly)))
            .merge(provider_routes.layer(limiter(&config.limiters.member_write, Scope::WritesOnly)))
            .route("/static/{*path}", get(views::static_file))
            .fallback(move |req: axum::extract::Request| fallback(legacy.clone(), req))
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
            // Renews an expired session before any handler (or the SvelteKit
            // proxy) reads the cookie. Inside the Origin check: a refused
            // cross-site request never rotates a token.
            .layer(middleware::from_fn_with_state(
                state.clone(),
                session::renew,
            ))
            .layer(Extension(session::CookiePolicy {
                secure: config.https,
            }))
            // Inside the renderer so a refused cross-site request gets a coded body.
            .layer(middleware::from_fn(move |req, next| {
                origin::enforce(allowed_origins.clone(), req, next)
            }))
            .layer(error_renderer)
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
            .with_state(state)
    }
}

/// Echoed on every response, so a user's bug report can be matched to the log
/// lines of that exact request. A client-supplied value is kept as-is.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

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

/// Anything no route matched: Rust-owned paths get a coded 404; pages go to
/// SvelteKit while it still serves them, otherwise 404 too.
async fn fallback(
    legacy: Option<LegacyProxy>,
    req: axum::extract::Request,
) -> axum::response::Response {
    match legacy {
        Some(proxy) if !is_rust_only(req.uri().path()) => match proxy.forward(req).await {
            Ok(response) => response,
            Err(e) => e.into_response(),
        },
        _ => Error::new(ErrorCode::RouteNotFound).into_response(),
    }
}
