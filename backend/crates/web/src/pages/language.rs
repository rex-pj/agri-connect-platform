//! POST /language — the language choice, from the top bar's menu, the
//! one-time offer and `molecules/language_switch.html`. A plain form, so it
//! works without JS: it keeps the choice in the `lang` cookie for a year and
//! goes back to the page it was sent from, so the address bar never shows
//! anything but that page.

use axum::{
    http::{header, HeaderValue},
    response::Response,
    Extension,
};
use domain::{Error, ErrorCode};
use serde::Deserialize;

use crate::{
    extract::ApiForm, locale::Lang, middleware::language::LANG_COOKIE, pages::see_other, routes,
    session::CookiePolicy,
};

/// A year: the choice outlives any session, and a signed-out farmer keeps it.
const LANG_COOKIE_MAX_AGE_SECS: u32 = 365 * 24 * 3600;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct LanguageForm {
    pub lang: String,
    /// The page the switch was on (path and query).
    pub back: String,
}

/// Where to go back to: a path on this site, else home. Never another
/// origin (`//host`, `/\host`), so the form cannot send anyone away.
fn local_path(back: &str) -> Option<&str> {
    let back = back.trim();
    let mut chars = back.chars();
    let local = chars.next() == Some('/')
        && !matches!(chars.next(), Some('/' | '\\'))
        && !back.chars().any(|c| c.is_control() || c == '\\');
    local.then_some(back)
}

/// `back` when it is a path on this site, else home.
fn back_to(back: &str) -> Result<String, Error> {
    match local_path(back) {
        Some(path) => Ok(path.to_owned()),
        None => {
            routes::url("home", &[]).map_err(|e| Error::internal(format!("language links: {e}")))
        }
    }
}

pub async fn switch(
    Extension(policy): Extension<CookiePolicy>,
    ApiForm(form): ApiForm<LanguageForm>,
) -> Result<Response, Error> {
    let lang =
        Lang::parse(form.lang.trim()).ok_or_else(|| Error::new(ErrorCode::InvalidRequestBody))?;
    let mut response = see_other(&back_to(&form.back)?)?;
    let secure = if policy.secure { "; Secure" } else { "" };
    let cookie = format!(
        "{LANG_COOKIE}={}; Path=/; Max-Age={LANG_COOKIE_MAX_AGE_SECS}; HttpOnly; SameSite=Lax{secure}",
        lang.code()
    );
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::local_path;

    #[test]
    fn only_paths_on_this_site_are_gone_back_to() {
        assert_eq!(
            local_path("/products?q=g%E1%BA%A1o"),
            Some("/products?q=g%E1%BA%A1o")
        );
        assert_eq!(local_path(" /admin/users "), Some("/admin/users"));
        for away in [
            "//evil.example",
            "/\\evil.example",
            "https://evil.example",
            "",
            "x",
            "/a\nb",
        ] {
            assert_eq!(local_path(away), None, "{away:?}");
        }
    }
}
