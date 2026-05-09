use std::{
    net::{IpAddr, SocketAddr},
    num::NonZeroU32,
    sync::Arc,
};

use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json,
};
use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota, RateLimiter};

use crate::constants::{AUTH_RATE_LIMIT_PER_MIN, PUBLIC_WRITE_RATE_LIMIT_PER_MIN};

pub type SharedLimiter =
    Arc<RateLimiter<IpAddr, DefaultKeyedStateStore<IpAddr>, DefaultClock>>;

/// Auth endpoint limiter — see `constants::AUTH_RATE_LIMIT_PER_MIN`.
pub fn new_auth_limiter() -> SharedLimiter {
    let quota = Quota::per_minute(NonZeroU32::new(AUTH_RATE_LIMIT_PER_MIN).unwrap());
    Arc::new(RateLimiter::keyed(quota))
}

/// Public-write limiter (contact form etc.) — see `constants::PUBLIC_WRITE_RATE_LIMIT_PER_MIN`.
pub fn new_public_write_limiter() -> SharedLimiter {
    let quota = Quota::per_minute(NonZeroU32::new(PUBLIC_WRITE_RATE_LIMIT_PER_MIN).unwrap());
    Arc::new(RateLimiter::keyed(quota))
}

/// Tower middleware function: enforce rate limit by client IP.
///
/// The `SharedLimiter` must be inserted as an Axum `Extension` on the same
/// router (or a parent router) before this middleware runs.
pub async fn enforce(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(limiter): Extension<SharedLimiter>,
    req: Request,
    next: Next,
) -> Response {
    if limiter.check_key(&addr.ip()).is_err() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "message": "Quá nhiều yêu cầu. Vui lòng thử lại sau."
            })),
        )
            .into_response();
    }
    next.run(req).await
}
