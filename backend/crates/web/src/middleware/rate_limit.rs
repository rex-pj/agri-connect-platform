//! Request budgets per client address, attached per router area (see
//! `app_routing`). The counting itself is behind the `RateLimiter` port.

use std::{
    net::{IpAddr, Ipv6Addr},
    sync::Arc,
};

use application::ports::RateLimiter;
use axum::{
    extract::Request,
    http::Method,
    middleware::Next,
    response::{IntoResponse, Response},
};
use domain::{Error, ErrorCode};
use infrastructure::rate_limiter::InProcessRateLimiter;

use crate::{
    constants::{
        AUTH_RATE_LIMIT_PER_MIN, MEMBER_WRITE_RATE_LIMIT_PER_MIN, PUBLIC_SEARCH_RATE_LIMIT_PER_MIN,
        PUBLIC_WRITE_RATE_LIMIT_PER_MIN, RATE_LIMIT_IPV6_PREFIX_BITS, UPLOAD_RATE_LIMIT_PER_MIN,
    },
    middleware::client_ip::ClientIp,
};

/// One budget per router area, so a burst of admin edits never eats into the
/// login budget and vice versa.
#[derive(Clone)]
pub struct Limiters {
    /// Login, registration, password reset, first-admin setup.
    pub auth: Arc<dyn RateLimiter>,
    /// Anonymous writes: the contact form.
    pub public_write: Arc<dyn RateLimiter>,
    /// Searches: text and near-me, pages and JSON.
    pub public_search: Arc<dyn RateLimiter>,
    /// Signed-in writes: admin and provider dashboard changes.
    pub member_write: Arc<dyn RateLimiter>,
    /// Image uploads (decoding is CPU work).
    pub upload: Arc<dyn RateLimiter>,
}

impl Limiters {
    pub fn in_process() -> Self {
        Self {
            auth: Arc::new(InProcessRateLimiter::per_minute(AUTH_RATE_LIMIT_PER_MIN)),
            public_write: Arc::new(InProcessRateLimiter::per_minute(
                PUBLIC_WRITE_RATE_LIMIT_PER_MIN,
            )),
            public_search: Arc::new(InProcessRateLimiter::per_minute(
                PUBLIC_SEARCH_RATE_LIMIT_PER_MIN,
            )),
            member_write: Arc::new(InProcessRateLimiter::per_minute(
                MEMBER_WRITE_RATE_LIMIT_PER_MIN,
            )),
            upload: Arc::new(InProcessRateLimiter::per_minute(UPLOAD_RATE_LIMIT_PER_MIN)),
        }
    }

    pub fn forget_idle(&self) {
        self.auth.forget_idle();
        self.public_write.forget_idle();
        self.public_search.forget_idle();
        self.member_write.forget_idle();
        self.upload.forget_idle();
    }
}

/// Whose budget a request from `ip` counts against: the address itself for
/// IPv4 (also when written as an IPv4-mapped IPv6 address), the /64 for IPv6.
pub fn budget_key(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => {
                let mask = u128::MAX << (u128::BITS - RATE_LIMIT_IPV6_PREFIX_BITS);
                IpAddr::V6(Ipv6Addr::from(u128::from(v6) & mask))
            }
        },
    }
}

#[derive(Clone, Copy)]
pub enum Scope {
    AllRequests,
    /// Reads are free: browsing an admin list must never be throttled.
    WritesOnly,
}

/// Keys on the client resolved by `client_ip::resolve`; without it (a router
/// built without that layer) the request is refused rather than counted
/// against a shared fallback key.
pub async fn enforce(
    limiter: Arc<dyn RateLimiter>,
    scope: Scope,
    req: Request,
    next: Next,
) -> Response {
    let counted = match scope {
        Scope::AllRequests => true,
        Scope::WritesOnly => !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS),
    };
    if counted {
        let Some(ClientIp(ip)) = req.extensions().get::<ClientIp>().copied() else {
            return Error::internal("rate limiter ran without client_ip::resolve").into_response();
        };
        if !limiter.allow(budget_key(ip)) {
            return Error::new(ErrorCode::TooManyRequests).into_response();
        }
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap_or(IpAddr::from([255, 255, 255, 255]))
    }

    #[test]
    fn an_ipv6_client_is_its_prefix_and_an_ipv4_client_its_address() {
        assert_eq!(budget_key(ip("203.0.113.7")), ip("203.0.113.7"));
        assert_eq!(budget_key(ip("::ffff:203.0.113.7")), ip("203.0.113.7"));
        assert_eq!(
            budget_key(ip("2001:db8:1:2:aaaa:bbbb:cccc:dddd")),
            ip("2001:db8:1:2::")
        );
        assert_eq!(
            budget_key(ip("2001:db8:1:2::1")),
            budget_key(ip("2001:db8:1:2:ffff:ffff:ffff:ffff")),
            "one subscriber, one budget"
        );
        assert_ne!(
            budget_key(ip("2001:db8:1:2::1")),
            budget_key(ip("2001:db8:1:3::1"))
        );
    }
}
