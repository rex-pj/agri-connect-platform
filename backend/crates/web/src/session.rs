//! Browser sessions: two httpOnly cookies, `auth_token` (the 1-hour access
//! JWT every extractor reads) and `refresh_token` (opaque, rotated on use).
//!
//! `SameSite=Lax`, not `Strict`: with Strict, a farmer opening a link from Zalo
//! arrives signed out. Lax still keeps the cookies off cross-site POSTs, and
//! every non-GET also passes the Origin check (`middleware::origin`).
//!
//! `renew` runs before the handlers: when the access token is missing or no
//! longer valid but a refresh token is present, it rotates the pair and
//! rewrites this request's `Cookie` header, so the handler sees a live
//! session without a second round trip.

use application::{
    constants::JWT_EXPIRY_HOURS,
    usecases::auth_usecase::{Renewal, SessionTokens},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use chrono::Utc;
use domain::Error;

use crate::{
    app_state::AppState, cookies::request_cookie, middleware::auth::SESSION_COOKIE,
    pages::see_other, routes,
};

pub const REFRESH_COOKIE: &str = "refresh_token";

/// Cookie attributes that depend on the deployment; set by the router from
/// HTTPS_ENABLED.
#[derive(Debug, Clone, Copy, Default)]
pub struct CookiePolicy {
    /// `Secure`: only behind TLS, or plain-HTTP development could not log in.
    /// Behind TLS the two session cookies also take the `__Host-` prefix:
    /// the browser then keeps them only as set by this very host (Secure,
    /// `Path=/`, no `Domain`), so no other site under the domain can plant
    /// a session of its choosing.
    pub secure: bool,
}

/// The session cookies behind TLS: the plain names with the __Host- prefix.
const HOST_SESSION_COOKIE: &str = "__Host-auth_token";
const HOST_REFRESH_COOKIE: &str = "__Host-refresh_token";

impl CookiePolicy {
    /// The access token's cookie: `__Host-auth_token` behind TLS.
    pub fn session_name(&self) -> &'static str {
        if self.secure {
            HOST_SESSION_COOKIE
        } else {
            SESSION_COOKIE
        }
    }

    /// The refresh token's cookie: `__Host-refresh_token` behind TLS.
    pub fn refresh_name(&self) -> &'static str {
        if self.secure {
            HOST_REFRESH_COOKIE
        } else {
            REFRESH_COOKIE
        }
    }

    fn cookie(&self, name: &str, value: &str, max_age_secs: i64) -> Option<HeaderValue> {
        let secure = if self.secure { "; Secure" } else { "" };
        HeaderValue::from_str(&format!(
            "{name}={value}; Path=/; Max-Age={max_age_secs}; HttpOnly; SameSite=Lax{secure}"
        ))
        .ok()
    }

    /// Both cookies for a session that just started or was renewed.
    pub fn set(&self, response: &mut Response, tokens: &SessionTokens) {
        let refresh_secs = (tokens.refresh_expires_at - Utc::now())
            .num_seconds()
            .max(0);
        let pairs = [
            (
                self.session_name(),
                tokens.access.as_str(),
                chrono::Duration::hours(JWT_EXPIRY_HOURS).num_seconds(),
            ),
            (self.refresh_name(), tokens.refresh.as_str(), refresh_secs),
        ];
        for (name, value, age) in pairs {
            if let Some(v) = self.cookie(name, value, age) {
                response.headers_mut().append(header::SET_COOKIE, v);
            }
        }
    }

    /// Expires both cookies.
    pub fn clear(&self, response: &mut Response) {
        for name in [self.session_name(), self.refresh_name()] {
            if let Some(v) = self.cookie(name, "", 0) {
                response.headers_mut().append(header::SET_COOKIE, v);
            }
        }
    }
}

/// Requests that never need a session: assets (the favicon too) and probes. Skipping them keeps
/// a page's dozen asset requests from racing to rotate the same token.
fn needs_session(path: &str) -> bool {
    !crate::routes::is_asset_path(path)
}

/// `Cookie` with the two session cookies replaced by the new pair, every
/// other cookie kept as it was.
fn with_session_cookies(
    headers: &HeaderMap,
    policy: CookiePolicy,
    tokens: &SessionTokens,
) -> Option<HeaderValue> {
    let (session, refresh) = (policy.session_name(), policy.refresh_name());
    let mut pairs: Vec<String> = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .map(str::trim)
        .filter(|p| {
            let name = p.split_once('=').map_or(*p, |(k, _)| k);
            !p.is_empty() && name != session && name != refresh
        })
        .map(str::to_owned)
        .collect();
    pairs.push(format!("{session}={}", tokens.access));
    pairs.push(format!("{refresh}={}", tokens.refresh));
    HeaderValue::from_str(&pairs.join("; ")).ok()
}

pub async fn renew(
    State(state): State<AppState>,
    Extension(policy): Extension<CookiePolicy>,
    mut req: Request,
    next: Next,
) -> Response {
    let headers = req.headers();
    // API clients send a bearer token and manage their own sessions.
    let bearer = headers.contains_key(header::AUTHORIZATION);
    let refresh = request_cookie(headers, policy.refresh_name()).map(str::to_owned);
    let access_ok = request_cookie(headers, policy.session_name())
        .is_some_and(|t| state.token_service.verify(t).is_ok());
    let Some(refresh) =
        refresh.filter(|_| !bearer && !access_ok && needs_session(req.uri().path()))
    else {
        return next.run(req).await;
    };

    match state.usecases.auth.renew_session(&refresh).await {
        Ok(Renewal::Renewed(tokens)) => {
            if let Some(cookie) = with_session_cookies(req.headers(), policy, &tokens) {
                req.headers_mut().insert(header::COOKIE, cookie);
            }
            let mut response = next.run(req).await;
            policy.set(&mut response, &tokens);
            response
        }
        Ok(Renewal::Raced) => {
            // Another tab renewed first and set the new cookies; clearing
            // them here could arrive last and sign the visitor out.
            next.run(req).await
        }
        Ok(Renewal::Ended) => {
            // Spent, expired or revoked: drop the dead cookies so the browser
            // stops sending them, and carry on signed out.
            let mut response = next.run(req).await;
            policy.clear(&mut response);
            response
        }
        Err(e) => {
            // The database is unreachable: the page can still render signed
            // out; the session is retried on the next request.
            tracing::warn!(error = %e, "could not renew the session");
            next.run(req).await
        }
    }
}

async fn end_session(
    state: &AppState,
    policy: CookiePolicy,
    headers: &HeaderMap,
) -> Result<(), Error> {
    match request_cookie(headers, policy.refresh_name()) {
        Some(raw) => state.usecases.auth.end_session(raw).await,
        None => Ok(()),
    }
}

async fn logout_to(
    state: &AppState,
    policy: CookiePolicy,
    headers: &HeaderMap,
    route: &str,
) -> Result<Response, Error> {
    end_session(state, policy, headers).await?;
    let target = routes::url(route, &[]).map_err(|e| Error::internal(format!("logout: {e}")))?;
    let mut response = see_other(&target)?;
    policy.clear(&mut response);
    Ok(response)
}

/// POST /logout — ends the session family and clears both cookies.
pub async fn logout(
    State(state): State<AppState>,
    Extension(policy): Extension<CookiePolicy>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    logout_to(&state, policy, &headers, "home").await
}

/// POST /admin/logout — the same, landing on the login page.
pub async fn admin_logout(
    State(state): State<AppState>,
    Extension(policy): Extension<CookiePolicy>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, Error> {
    logout_to(&state, policy, &headers, "login").await
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    fn tokens() -> SessionTokens {
        SessionTokens {
            access: "new.jwt".into(),
            refresh: "newrefresh".into(),
            refresh_expires_at: Utc::now() + Duration::days(30),
        }
    }

    #[test]
    fn the_request_cookie_gets_the_new_pair_and_keeps_the_rest() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("lang=vi; auth_token=old; refresh_token=spent; flash=x"),
        );
        let rewritten = with_session_cookies(&h, CookiePolicy::default(), &tokens());
        assert_eq!(
            rewritten.as_ref().and_then(|v| v.to_str().ok()),
            Some("lang=vi; flash=x; auth_token=new.jwt; refresh_token=newrefresh")
        );
    }

    #[test]
    fn session_cookies_are_lax_httponly_and_secure_only_behind_tls() {
        let mut res = ().into_response();
        CookiePolicy { secure: true }.set(&mut res, &tokens());
        let set: Vec<&str> = res
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect();
        assert_eq!(set.len(), 2);
        assert!(set.iter().all(|c| c.contains("HttpOnly")
            && c.contains("SameSite=Lax")
            && c.ends_with("; Secure")));
        assert!(set.first().is_some_and(
            |c| c.starts_with("__Host-auth_token=new.jwt;") && c.contains("Max-Age=3600")
        ));

        let mut plain = ().into_response();
        CookiePolicy { secure: false }.clear(&mut plain);
        assert!(plain
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .all(|c| c.contains("Max-Age=0") && !c.contains("Secure")));
    }

    #[test]
    fn behind_tls_the_session_cookies_are_host_only() {
        let tls = CookiePolicy { secure: true };
        assert_eq!(
            (tls.session_name(), tls.refresh_name()),
            ("__Host-auth_token", "__Host-refresh_token")
        );
        assert_eq!(HOST_SESSION_COOKIE, format!("__Host-{SESSION_COOKIE}"));
        assert_eq!(HOST_REFRESH_COOKIE, format!("__Host-{REFRESH_COOKIE}"));
        let plain = CookiePolicy::default();
        assert_eq!(
            (plain.session_name(), plain.refresh_name()),
            ("auth_token", "refresh_token")
        );
        let mut res = ().into_response();
        tls.set(&mut res, &tokens());
        for cookie in res.headers().get_all(header::SET_COOKIE) {
            let cookie = cookie.to_str().unwrap_or_default();
            // What a browser requires of a __Host- cookie.
            assert!(cookie.starts_with("__Host-"), "{cookie}");
            assert!(cookie.contains("; Path=/;") && cookie.ends_with("; Secure"));
            assert!(!cookie.contains("Domain"), "{cookie}");
        }
        // The request's other cookies are kept, the old pair replaced.
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("lang=vi; __Host-auth_token=old; __Host-refresh_token=spent"),
        );
        assert_eq!(
            with_session_cookies(&h, tls, &tokens())
                .as_ref()
                .and_then(|v| v.to_str().ok()),
            Some("lang=vi; __Host-auth_token=new.jwt; __Host-refresh_token=newrefresh")
        );
    }

    #[test]
    fn assets_never_trigger_a_renewal() {
        assert!(!needs_session("/static/css/app.css"));
        assert!(!needs_session("/files/abc/thumb"));
        assert!(!needs_session("/favicon.ico"));
        assert!(needs_session("/admin/dashboard"));
        assert!(needs_session("/api/users/me"));
    }
}
