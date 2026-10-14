//! Server-rendered pages. Each module is one route (or a family of them):
//! it builds a view model from use cases and hands it to `render_page`.

pub mod cards;
pub mod home;
pub mod product;

use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use domain::Error;

/// 301 to `location`. Not axum's `Redirect::permanent`, which is a 308: old
/// crawlers and some link previews only treat 301 as "this page moved".
pub fn moved_permanently(location: &str) -> Result<Response, Error> {
    let value = HeaderValue::from_str(location)
        .map_err(|_| Error::internal(format!("redirect target {location:?} is not a header")))?;
    Ok((StatusCode::MOVED_PERMANENTLY, [(header::LOCATION, value)]).into_response())
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
