//! `MarkdownRenderer` over pulldown-cmark: the only file that names it.
//!
//! The text is parsed, every event that could put markup of the author's on
//! the page is rewritten before pulldown-cmark writes the HTML (which
//! escapes every text and attribute it writes), and the headings are given
//! anchors for the page's outline. Nothing typed as HTML passes: there is no
//! sanitizer to get wrong, since no raw HTML is ever written.

use std::collections::HashSet;

use application::{
    ports::{Heading, MarkdownRenderer, RenderedText},
    slug::{numbered, slugify},
};
use pulldown_cmark::{html, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// The anchor of a heading with no letter or digit in it.
const UNNAMED_HEADING: &str = "section";
/// The longest heading anchor, suffix included.
const HEADING_ID_MAX: usize = 80;
/// The address an image must start with: one of our own uploads.
const OWN_FILES: &str = "/files/";
/// The schemes a link may lead to, lower-cased.
const LINK_SCHEMES: [&str; 4] = ["https://", "http://", "mailto:", "tel:"];

#[derive(Debug, Clone, Copy, Default)]
pub struct PulldownMarkdownRenderer;

impl MarkdownRenderer for PulldownMarkdownRenderer {
    fn render(&self, source: &str) -> RenderedText {
        let (events, outline) = with_anchors(safe_events(source));
        let mut html = String::with_capacity(source.len().saturating_mul(2));
        html::push_html(&mut html, events.into_iter());
        RenderedText { html, outline }
    }
}

/// Tables and ~~strikethrough~~ beside CommonMark; no raw-HTML extension,
/// no heading attributes (an author never sets a class or an id).
fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
}

/// A link may lead to the web, an email, a phone, a page of this site or an
/// anchor of this page; never `javascript:`, `data:` or another host's
/// protocol-relative `//`.
fn safe_link(dest: &str) -> bool {
    let dest = dest.trim();
    let lower = dest.to_ascii_lowercase();
    LINK_SCHEMES.iter().any(|s| lower.starts_with(s))
        || (dest.starts_with('/') && !dest.starts_with("//") && !dest.starts_with("/\\"))
        || dest.starts_with('#')
}

fn safe_image(dest: &str) -> bool {
    dest.starts_with(OWN_FILES)
}

/// The text's events with every typed HTML turned into text (an HTML block
/// as a paragraph of it) and every link or image that leads elsewhere than
/// `safe_link` / `safe_image` taken away, its words kept.
fn safe_events(source: &str) -> Vec<Event<'_>> {
    let mut out = Vec::new();
    // For each link or image open: whether it was kept, so its end is too.
    let mut open: Vec<bool> = Vec::new();
    for event in Parser::new_ext(source, options()) {
        match event {
            Event::Html(text) | Event::InlineHtml(text) => out.push(Event::Text(text)),
            Event::Start(Tag::HtmlBlock) => out.push(Event::Start(Tag::Paragraph)),
            Event::End(TagEnd::HtmlBlock) => out.push(Event::End(TagEnd::Paragraph)),
            Event::Start(tag @ (Tag::Link { .. } | Tag::Image { .. })) => {
                let kept = match &tag {
                    Tag::Link { dest_url, .. } => safe_link(dest_url),
                    Tag::Image { dest_url, .. } => safe_image(dest_url),
                    _ => false,
                };
                open.push(kept);
                if kept {
                    out.push(Event::Start(tag));
                }
            }
            Event::End(end @ (TagEnd::Link | TagEnd::Image)) => {
                if open.pop().unwrap_or(false) {
                    out.push(Event::End(end));
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// A `#` is drawn as a `##`: the page's title is its one `<h1>`.
fn demoted(level: HeadingLevel) -> HeadingLevel {
    match level {
        HeadingLevel::H1 => HeadingLevel::H2,
        other => other,
    }
}

fn level_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// The anchor of a heading worded `text`, never one the text already has.
fn unique_anchor(text: &str, taken: &mut HashSet<String>) -> String {
    let slug = slugify(text);
    let base = if slug.is_empty() {
        UNNAMED_HEADING
    } else {
        &slug
    };
    let mut n = 1;
    loop {
        let candidate = numbered(base, n, HEADING_ID_MAX);
        if taken.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

/// Each heading demoted and given its anchor; the `##` and `###` ones
/// listed as the outline.
fn with_anchors(events: Vec<Event<'_>>) -> (Vec<Event<'_>>, Vec<Heading>) {
    let mut out = Vec::with_capacity(events.len());
    let mut outline = Vec::new();
    let mut taken = HashSet::new();
    let mut events = events.into_iter();
    while let Some(event) = events.next() {
        let Event::Start(Tag::Heading { level, .. }) = event else {
            out.push(event);
            continue;
        };
        let level = demoted(level);
        let inner: Vec<Event<'_>> = events
            .by_ref()
            .take_while(|e| !matches!(e, Event::End(TagEnd::Heading(_))))
            .collect();
        let text: String = inner
            .iter()
            .filter_map(|e| match e {
                Event::Text(t) | Event::Code(t) => Some(t.as_ref()),
                _ => None,
            })
            .collect();
        let id = unique_anchor(&text, &mut taken);
        if matches!(level, HeadingLevel::H2 | HeadingLevel::H3) {
            outline.push(Heading {
                level: level_number(level),
                id: id.clone(),
                text: text.trim().to_owned(),
            });
        }
        out.push(Event::Start(Tag::Heading {
            level,
            id: Some(CowStr::from(id)),
            classes: Vec::new(),
            attrs: Vec::new(),
        }));
        out.extend(inner);
        out.push(Event::End(TagEnd::Heading(level)));
    }
    (out, outline)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(source: &str) -> RenderedText {
        PulldownMarkdownRenderer.render(source)
    }

    #[test]
    fn typed_html_is_shown_as_text() {
        for source in [
            "<script>alert(1)</script>",
            "Hello <img src=x onerror=alert(1)> there",
            "<div onclick=\"x()\">block</div>\n\nafter",
            "<a href=\"javascript:alert(1)\">x</a>",
        ] {
            let html = render(source).html;
            assert!(!html.contains("<script"), "{html}");
            assert!(!html.contains("<img"), "{html}");
            assert!(!html.contains("<div"), "{html}");
            assert!(!html.contains("<a "), "{html}");
            assert!(html.contains("&lt;"), "{html}");
        }
    }

    #[test]
    fn only_safe_links_are_kept_with_their_words() {
        let html = render(
            "[a](https://example.org) [b](/info/terms) [c](#part) [d](tel:0901234567) \
             [e](mailto:x@example.org) [f](javascript:alert(1)) [g](JavaScript:x) \
             [h](data:text/html,x) [i](//evil.example) [j](vbscript:x)",
        )
        .html;
        for kept in [
            "href=\"https://example.org\"",
            "href=\"/info/terms\"",
            "href=\"#part\"",
            "href=\"tel:0901234567\"",
            "href=\"mailto:x@example.org\"",
        ] {
            assert!(html.contains(kept), "{kept} in {html}");
        }
        for gone in ["javascript", "JavaScript", "data:", "//evil", "vbscript"] {
            assert!(!html.contains(gone), "{gone} in {html}");
        }
        for word in ["f", "g", "h", "i", "j"] {
            assert!(html.contains(&format!(" {word}")), "{word} in {html}");
        }
    }

    #[test]
    fn only_our_uploads_are_shown_as_images() {
        let html = render("![farm](/files/abc) ![x](https://tracker.example/p.gif)").html;
        assert!(html.contains("src=\"/files/abc\""), "{html}");
        assert!(!html.contains("tracker"), "{html}");
        assert_eq!(html.matches("<img").count(), 1, "{html}");
    }

    #[test]
    fn headings_get_unique_anchors_and_make_the_outline() {
        let rendered =
            render("# Scope\n\ntext\n\n## Scope\n\n### `Code` alone\n\n#### Small\n\n## !!!");
        let ids: Vec<&str> = rendered.outline.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["scope", "scope-2", "code-alone", "section"]);
        let levels: Vec<u8> = rendered.outline.iter().map(|h| h.level).collect();
        assert_eq!(levels, [2, 2, 3, 2]);
        assert!(!rendered.html.contains("<h1"), "{}", rendered.html);
        assert!(
            rendered.html.contains("<h2 id=\"scope\">"),
            "{}",
            rendered.html
        );
        assert!(
            rendered.html.contains("<h4 id=\"small\">"),
            "{}",
            rendered.html
        );
        assert_eq!(
            rendered.outline.first().map(|h| h.text.as_str()),
            Some("Scope")
        );
    }

    #[test]
    fn tables_and_lists_render() {
        let html = render("| a | b |\n|---|---|\n| 1 | 2 |\n\n- one\n- ~~two~~").html;
        assert!(html.contains("<table>"), "{html}");
        assert!(html.contains("<li>one</li>"), "{html}");
        assert!(html.contains("<del>two</del>"), "{html}");
    }
}
