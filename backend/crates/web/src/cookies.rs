//! Request cookies, and the one-time "flash" notice a form leaves for the page
//! it redirects to (Post/Redirect/Get). A flash is a cookie rather than a
//! `?sent=1` query: a shared or reloaded URL must not claim "sent" again.

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap, HeaderValue},
    response::Response,
};
use serde::Serialize;

/// The value of cookie `name`, if present and non-empty.
pub fn request_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
        .filter(|v| !v.is_empty())
}

const FLASH_COOKIE: &str = "flash";
/// Long enough to survive the redirect on a slow connection, short enough
/// that an abandoned one does not show up on some later page.
const FLASH_MAX_AGE_SECS: u32 = 60;

/// Notices a form can leave for the next page. The wire names are cookie
/// values; the page template branches on the serialized name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Flash {
    ContactSent,
    PasswordReset,
    DemoLoaded,
    DemoRemoved,
    PasswordChanged,
    RoleSaved,
}

impl Flash {
    const ALL: [Flash; 6] = [
        Flash::ContactSent,
        Flash::PasswordReset,
        Flash::DemoLoaded,
        Flash::DemoRemoved,
        Flash::PasswordChanged,
        Flash::RoleSaved,
    ];

    const fn as_str(self) -> &'static str {
        match self {
            Flash::ContactSent => "contact_sent",
            Flash::PasswordReset => "password_reset",
            Flash::DemoLoaded => "demo_loaded",
            Flash::DemoRemoved => "demo_removed",
            Flash::PasswordChanged => "password_changed",
            Flash::RoleSaved => "role_saved",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.as_str() == value)
    }

    /// Adds the cookie to a redirect. Not `Secure`: it carries no secret, and
    /// it must also work on plain-HTTP development servers.
    pub fn set_on(self, response: &mut Response) {
        let cookie = format!(
            "{FLASH_COOKIE}={}; Path=/; Max-Age={FLASH_MAX_AGE_SECS}; HttpOnly; SameSite=Lax",
            self.as_str()
        );
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
}

/// The flash left for this request, if any. Call `clear_on` on the page's
/// response so it shows once.
pub struct TakeFlash(pub Option<Flash>);

impl TakeFlash {
    pub fn clear_on(&self, response: &mut Response) {
        if self.0.is_some() {
            response.headers_mut().append(
                header::SET_COOKIE,
                HeaderValue::from_static("flash=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax"),
            );
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for TakeFlash {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            request_cookie(&parts.headers, FLASH_COOKIE).and_then(Flash::parse),
        ))
    }
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;

    use super::*;

    fn headers(cookie: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(cookie) {
            h.insert(header::COOKIE, v);
        }
        h
    }

    #[test]
    fn cookies_are_found_by_exact_name_only() {
        let h = headers("theme=dark; auth_token=abc.def; xflash=1; flash=");
        assert_eq!(request_cookie(&h, "auth_token"), Some("abc.def"));
        assert_eq!(request_cookie(&h, "flash"), None, "empty is absent");
    }

    #[test]
    fn a_flash_round_trips_and_clears() {
        let mut res = ().into_response();
        Flash::ContactSent.set_on(&mut res);
        let set = res.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap_or_default();
        assert!(set.starts_with("flash=contact_sent;") && set.contains("HttpOnly"));

        let taken = TakeFlash(
            request_cookie(&headers("flash=contact_sent"), FLASH_COOKIE).and_then(Flash::parse),
        );
        assert_eq!(taken.0, Some(Flash::ContactSent));
        let mut page = ().into_response();
        taken.clear_on(&mut page);
        assert!(page.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap_or_default()
            .contains("Max-Age=0"));
        // A forged value is ignored.
        assert_eq!(Flash::parse("<script>"), None);
        for flash in Flash::ALL {
            assert_eq!(Flash::parse(flash.as_str()), Some(flash));
        }
    }
}
