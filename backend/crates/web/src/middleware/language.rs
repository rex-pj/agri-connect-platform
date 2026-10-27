//! Serves every request in the visitor's language: the `lang` cookie the
//! language switch sets, else the default. Outermost of the page layers, so
//! error bodies are worded in it too.

use axum::{extract::Request, middleware::Next, response::Response};

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
