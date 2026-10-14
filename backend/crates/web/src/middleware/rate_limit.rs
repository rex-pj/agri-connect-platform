use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use axum::{
    extract::{ConnectInfo, Request},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use domain::{Error, ErrorCode};
use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota, RateLimiter};

use crate::constants::{AUTH_RATE_LIMIT_PER_MIN, PUBLIC_WRITE_RATE_LIMIT_PER_MIN};

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

/// Keys on the TCP peer address. Behind a reverse proxy every request shares
/// the proxy's address; slice S2 adds `TRUSTED_PROXY_HOPS` to fix that.
pub async fn enforce(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(limiter): Extension<SharedLimiter>,
    req: Request,
    next: Next,
) -> Response {
    if limiter.check_key(&addr.ip()).is_err() {
        return Error::new(ErrorCode::TooManyRequests).into_response();
    }
    next.run(req).await
}
