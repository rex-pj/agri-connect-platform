//! Which address a request came from. Behind Caddy every TCP peer is the
//! Caddy container, so keying the rate limiter on the peer would put every
//! user in one shared bucket: ten bad logins anywhere would lock everyone out.

use std::net::{IpAddr, SocketAddr};

use axum::{
    extract::{ConnectInfo, Request},
    middleware::Next,
    response::Response,
};

/// The resolved client address, inserted into every request's extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientIp(pub IpAddr);

/// Each proxy appends the address it received the request from, so with
/// `hops` trusted proxies the client is the `hops`-th entry from the right.
/// Entries further left were written by the client and may be forged; they are
/// never used. Anything unexpected (header missing, too short, not an address)
/// falls back to the TCP peer.
pub fn client_ip(peer: IpAddr, forwarded_for: Option<&str>, hops: usize) -> IpAddr {
    if hops == 0 {
        return peer;
    }
    let Some(header) = forwarded_for else {
        return peer;
    };
    let entries: Vec<&str> = header.split(',').map(str::trim).collect();
    entries
        .len()
        .checked_sub(hops)
        .and_then(|i| entries.get(i))
        .and_then(|e| e.parse().ok())
        .unwrap_or(peer)
}

/// The text a use case tells one client from another by (wrong passwords,
/// job posts): the address that client's rate-limit budget is kept under
/// (an IPv6 client is its /64), or empty when none was resolved. Never
/// stored; the use cases hash it before keeping it in memory.
pub fn client_key(ip: Option<&ClientIp>) -> String {
    ip.map(|ClientIp(ip)| crate::middleware::rate_limit::budget_key(*ip).to_string())
        .unwrap_or_default()
}

pub async fn resolve(hops: usize, mut req: Request, next: Next) -> Response {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
        .unwrap_or(IpAddr::from([0, 0, 0, 0]));
    // With several X-Forwarded-For headers, proxies append to the last one.
    let forwarded = req
        .headers()
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>()
        .join(",");
    let ip = client_ip(
        peer,
        (!forwarded.is_empty()).then_some(forwarded.as_str()),
        hops,
    );
    req.extensions_mut().insert(ClientIp(ip));
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap_or(IpAddr::from([255, 255, 255, 255]))
    }

    const CADDY: &str = "172.18.0.5";

    #[test]
    fn no_trusted_proxy_means_the_peer() {
        assert_eq!(client_ip(ip(CADDY), Some("1.2.3.4"), 0), ip(CADDY));
    }

    #[test]
    fn one_hop_takes_the_entry_caddy_appended() {
        assert_eq!(client_ip(ip(CADDY), Some("1.2.3.4"), 1), ip("1.2.3.4"));
    }

    #[test]
    fn a_forged_prefix_is_ignored() {
        // The client sent "X-Forwarded-For: 6.6.6.6"; Caddy appended the real address.
        assert_eq!(
            client_ip(ip(CADDY), Some("6.6.6.6, 1.2.3.4"), 1),
            ip("1.2.3.4")
        );
    }

    #[test]
    fn two_hops_skip_the_inner_proxy() {
        assert_eq!(
            client_ip(ip(CADDY), Some("1.2.3.4, 10.0.0.2"), 2),
            ip("1.2.3.4")
        );
    }

    #[test]
    fn missing_short_or_garbage_headers_fall_back_to_the_peer() {
        assert_eq!(client_ip(ip(CADDY), None, 1), ip(CADDY));
        assert_eq!(client_ip(ip(CADDY), Some("1.2.3.4"), 2), ip(CADDY));
        assert_eq!(client_ip(ip(CADDY), Some("not-an-ip"), 1), ip(CADDY));
    }

    #[test]
    fn ipv6_clients_work() {
        assert_eq!(
            client_ip(ip(CADDY), Some("2001:db8::1"), 1),
            ip("2001:db8::1")
        );
    }
}
