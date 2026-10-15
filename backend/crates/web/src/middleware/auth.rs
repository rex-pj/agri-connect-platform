//! Who is calling. Extracts the session; never decides access. Every access
//! decision is a permission check inside a use case (`actor.require(KEY)`), so
//! an endpoint cannot be opened up by forgetting a guard here.

use application::authz::Actor;
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use domain::{Error, ErrorCode};

use crate::{app_state::AppState, cookies::request_cookie};

/// The session cookie. Set today by the SvelteKit app (httpOnly), read here so
/// Rust pages and islands share the same login during the migration.
pub const SESSION_COOKIE: &str = "auth_token";

/// The token from `Authorization: Bearer` (the SvelteKit server calling the
/// API) or else from the session cookie (a browser on a Rust page).
fn session_token(parts: &Parts) -> Option<&str> {
    let bearer = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    bearer.or_else(|| request_cookie(&parts.headers, SESSION_COOKIE))
}

/// `None` for an anonymous caller, and for a valid token whose account was
/// deleted or deactivated since. A token that is present but forged or
/// expired is rejected with `invalid_token`, so a client can tell "log in
/// again" from "not logged in".
pub struct MaybeActor(pub Option<Actor>);

impl FromRequestParts<AppState> for MaybeActor {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(token) = session_token(parts) else {
            return Ok(Self(None));
        };
        let claims = state.token_service.verify(token)?;
        Ok(Self(state.usecases.auth.actor(claims.user_id).await?))
    }
}

/// Who is looking at a page. Unlike `MaybeActor`, an expired or forged session
/// cookie is simply "nobody": a stale cookie must never make a page fail.
pub struct Viewer(pub Option<Actor>);

impl FromRequestParts<AppState> for Viewer {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match MaybeActor::from_request_parts(parts, state).await {
            Ok(MaybeActor(actor)) => Ok(Self(actor)),
            Err(e) if e.code() == ErrorCode::InvalidToken => Ok(Self(None)),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::Request;

    use super::*;

    fn parts(headers: &[(&str, &str)]) -> Parts {
        let mut req = Request::builder();
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        req.body(())
            .map(|r| r.into_parts().0)
            .unwrap_or_else(|_| Request::new(()).into_parts().0)
    }

    #[test]
    fn bearer_wins_over_cookie() {
        let p = parts(&[
            ("authorization", "Bearer from-header"),
            ("cookie", "auth_token=from-cookie"),
        ]);
        assert_eq!(session_token(&p), Some("from-header"));
    }

    #[test]
    fn cookie_is_found_among_others() {
        let p = parts(&[("cookie", "theme=dark; auth_token=abc.def; lang=vi")]);
        assert_eq!(session_token(&p), Some("abc.def"));
    }

    #[test]
    fn similar_names_and_empty_values_are_not_sessions() {
        let p = parts(&[("cookie", "xauth_token=nope; auth_token=")]);
        assert_eq!(session_token(&p), None);
    }
}
