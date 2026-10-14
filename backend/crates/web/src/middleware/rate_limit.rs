use std::{net::IpAddr, sync::Arc};

use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use domain::{Error, ErrorCode};
use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota, RateLimiter};

use crate::{
    constants::{AUTH_RATE_LIMIT_PER_MIN, PUBLIC_WRITE_RATE_LIMIT_PER_MIN},
    middleware::client_ip::ClientIp,
};

pub type SharedLimiter = Arc<RateLimiter<IpAddr, DefaultKeyedStateStore<IpAddr>, DefaultClock>>;

pub fn new_auth_limiter() -> SharedLimiter {
    Arc::new(RateLimiter::keyed(Quota::per_minute(
        AUTH_RATE_LIMIT_PER_MIN,
    )))
}

pub fn new_public_write_limiter() -> SharedLimiter {
    Arc::new(RateLimiter::keyed(Quota::per_minute(
        PUBLIC_WRITE_RATE_LIMIT_PER_MIN,
    )))
}

/// Keys on the client address resolved by `client_ip::resolve`, so users behind
/// the same reverse proxy do not share one bucket.
pub async fn enforce(
    Extension(ClientIp(ip)): Extension<ClientIp>,
    Extension(limiter): Extension<SharedLimiter>,
    req: Request,
    next: Next,
) -> Response {
    if limiter.check_key(&ip).is_err() {
        return Error::new(ErrorCode::TooManyRequests).into_response();
    }
    next.run(req).await
}
