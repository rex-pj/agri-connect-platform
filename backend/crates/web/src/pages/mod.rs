//! Server-rendered pages. Each module is one route (or a family of them):
//! it builds a view model from use cases and hands it to `render_page`.

pub mod account;
pub mod admin;
pub mod auth;
pub mod cards;
pub mod dashboard;
pub mod directory;
pub mod explore;
pub mod forms;
pub mod home;
pub mod inbox;
pub mod language;
pub mod listing;
pub mod own_reviews;
pub mod own_services;
pub mod partner;
pub mod product;
pub mod product_form;
pub mod products;
pub mod profile_form;
pub mod provider;
pub mod search;
pub mod service;
pub mod service_form;
pub mod services;
pub mod setup;

use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use domain::{Error, ErrorCode};

use crate::{routes, slug::slugify};

/// A path id that is not a number is a missing record, not a bad request: a
/// person typed or pasted it.
pub fn parse_id(raw: &str, missing: ErrorCode) -> Result<i32, Error> {
    raw.parse::<i32>().map_err(|_| Error::new(missing))
}

/// `/{things}/{id}/{slug of name}` for `route` ("product", "provider").
pub fn canonical_url(route: &str, id: i32, name: &str) -> Result<String, Error> {
    let id = id.to_string();
    routes::url(route, &[("id", &id), ("slug", &slugify(name))])
        .map_err(|e| Error::internal(format!("{route} url: {e}")))
}

/// 301 to `location`. Not axum's `Redirect::permanent`, which is a 308: old
/// crawlers and some link previews only treat 301 as "this page moved".
pub fn moved_permanently(location: &str) -> Result<Response, Error> {
    redirect(StatusCode::MOVED_PERMANENTLY, location)
}

/// 303 after a form post: the browser follows with a GET, so reloading the
/// result page never re-submits the form.
pub fn see_other(location: &str) -> Result<Response, Error> {
    redirect(StatusCode::SEE_OTHER, location)
}

fn redirect(status: StatusCode, location: &str) -> Result<Response, Error> {
    let value = HeaderValue::from_str(location)
        .map_err(|_| Error::internal(format!("redirect target {location:?} is not a header")))?;
    Ok((status, [(header::LOCATION, value)]).into_response())
}

/// `text` on one line, cut to at most `max` characters with an ellipsis. For
/// meta descriptions, which search engines show as a single short snippet.
pub fn summary(text: &str, max: usize) -> Option<String> {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.is_empty() {
        return None;
    }
    if line.chars().count() <= max {
        return Some(line);
    }
    let cut: String = line.chars().take(max.saturating_sub(1)).collect();
    Some(format!("{}…", cut.trim_end()))
}

/// Who is looking, for counting demand: a signed-in admin, provider or
/// partner (never counted), or an anonymous visitor told apart by their
/// address, which the counter keeps in memory only.
pub fn visitor(
    shell: &crate::shell::Shell,
    ip: Option<&crate::middleware::client_ip::ClientIp>,
) -> application::inputs::Visitor {
    use application::inputs::Visitor;
    if shell.profile.as_ref().is_some_and(|p| p.signed_in) {
        Visitor::SignedIn
    } else {
        Visitor::Anonymous {
            key: ip.map(|i| i.0.to_string()).unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::summary;

    #[test]
    fn summaries_are_one_line_and_bounded_in_characters() {
        assert_eq!(
            summary("Gạo\nST25\t thơm", 50),
            Some("Gạo ST25 thơm".into())
        );
        assert_eq!(summary("   ", 50), None);
        // Counted in characters: Vietnamese letters are several bytes each.
        let long = "ạ".repeat(200);
        let s = summary(&long, 150).unwrap_or_default();
        assert_eq!(s.chars().count(), 150);
        assert!(s.ends_with('…'));
    }
}
