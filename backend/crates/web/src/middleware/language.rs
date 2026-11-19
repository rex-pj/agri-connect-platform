//! Serves every request in the visitor's language: the `lang` cookie the
//! language switch sets, else the default. Outermost of the page layers, so
//! error bodies are worded in it too.

use axum::{
    extract::Request,
    http::{header, HeaderMap},
    middleware::Next,
    response::Response,
};

use crate::{
    cookies::request_cookie,
    locale::{self, Lang},
};

/// Set by `POST /language` (`pages::language`).
pub const LANG_COOKIE: &str = "lang";

pub async fn scope_request(req: Request, next: Next) -> Response {
    let lang = request_cookie(req.headers(), LANG_COOKIE)
        .and_then(Lang::parse)
        .unwrap_or(Lang::DEFAULT);
    locale::scope(lang, next.run(req)).await
}

/// The language of ours the browser ranks highest in `Accept-Language`
/// (`en-US,en;q=0.9,vi;q=0.8` → English). Only ever offered, never applied:
/// many farmers' phones are set to English while they read Vietnamese.
pub fn preferred_language(headers: &HeaderMap) -> Option<Lang> {
    let header = headers.get(header::ACCEPT_LANGUAGE)?.to_str().ok()?;
    let mut ranked: Vec<(f32, Lang)> = header
        .split(',')
        .filter_map(|entry| {
            let mut parts = entry.split(';').map(str::trim);
            let tag = parts.next()?;
            let primary = tag.split('-').next()?.to_ascii_lowercase();
            let lang = Lang::parse(&primary)?;
            let q = parts
                .find_map(|p| p.strip_prefix("q="))
                .map_or(Some(1.0), |q| q.parse::<f32>().ok())?;
            (q > 0.0).then_some((q, lang))
        })
        .collect();
    // Stable: equal weights keep the browser's order.
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    ranked.first().map(|(_, lang)| *lang)
}

#[cfg(test)]
mod tests {
    use axum::http::{header, HeaderMap, HeaderValue};

    use super::preferred_language;
    use crate::locale::Lang;

    fn prefers(value: &str) -> Option<Lang> {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_str(value).unwrap_or(HeaderValue::from_static("")),
        );
        preferred_language(&headers)
    }

    #[test]
    fn the_best_ranked_language_of_ours_wins() {
        assert_eq!(prefers("en-US,en;q=0.9,vi;q=0.8"), Some(Lang::En));
        assert_eq!(prefers("vi-VN,vi;q=0.9,en;q=0.5"), Some(Lang::Vi));
        assert_eq!(prefers("fr-FR, en;q=0.4, vi;q=0.7"), Some(Lang::Vi));
        assert_eq!(prefers("fr, de"), None);
        assert_eq!(prefers("en;q=0, vi;q=0.1"), Some(Lang::Vi));
        assert_eq!(prefers("EN-gb"), Some(Lang::En));
        assert_eq!(preferred_language(&HeaderMap::new()), None);
    }
}
