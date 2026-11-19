//! The icons an admin may give a sector, an alert kind, a commodity, a
//! service or supply type: every name the vendored Font Awesome 6 defines
//! (aliases too), offered by the picker (`molecules/form_field.html`
//! `icon_select`) under that name. A name is stored as `fa-…`;
//! `atoms/icon.html` adds the style.

/// The style class an older form stored before the name (`fa-solid fa-cow`):
/// accepted and dropped, so such a row saves unchanged.
const STYLE_CLASS: &str = "fa-solid";

/// What every Font Awesome icon name starts with.
const PREFIX: &str = "fa-";

/// Every icon name of the vendored font (`icons/names.rs`, generated), sorted.
mod names;

pub use names::ICONS;

/// An icon as sent or stored, as the picker names it: trimmed, the style
/// class dropped, `None` for blank. Not checked against the list.
pub fn normalize(raw: &str) -> Option<&str> {
    let name = raw.trim();
    let name = match name.strip_prefix(STYLE_CLASS) {
        Some(rest) if rest.is_empty() || rest.starts_with(char::is_whitespace) => rest.trim(),
        _ => name,
    };
    (!name.is_empty()).then_some(name)
}

/// The listed name `name` (normalized) stands for: itself, or with its
/// `fa-` added (`cow` is `fa-cow`, as the box takes it typed without JS).
pub fn known(name: &str) -> Option<&'static str> {
    let name = name.to_lowercase();
    let full = if name.starts_with(PREFIX) {
        name
    } else {
        format!("{PREFIX}{name}")
    };
    ICONS
        .binary_search(&full.as_str())
        .ok()
        .and_then(|i| ICONS.get(i).copied())
}

/// The names matching `query`, best first, at most `limit`: every word of it
/// (split at spaces and dashes, `fa-` ignored) is in the name; the name
/// itself, then the names it starts, then those holding its words together,
/// then the rest, each alphabetical. A blank query gives the first names.
pub fn search(query: &str, limit: usize) -> Vec<&'static str> {
    let query = query.trim().to_lowercase();
    let query = query.strip_prefix(PREFIX).unwrap_or(&query);
    let words: Vec<&str> = query
        .split(|c: char| c.is_whitespace() || c == '-')
        .filter(|w| !w.is_empty())
        .collect();
    let phrase = words.join("-");
    let mut found: Vec<(u8, &'static str)> = ICONS
        .iter()
        .filter_map(|icon| {
            let bare = icon.strip_prefix(PREFIX).unwrap_or(icon);
            if !words.iter().all(|w| bare.contains(w)) {
                return None;
            }
            let rank = if bare == phrase {
                0
            } else if bare.starts_with(&phrase) {
                1
            } else if bare.contains(&phrase) {
                2
            } else {
                3
            };
            Some((rank, *icon))
        })
        .collect();
    found.sort_unstable();
    found
        .into_iter()
        .take(limit)
        .map(|(_, icon)| icon)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_sorted_once_and_named_as_stored() {
        assert!(
            ICONS.is_sorted_by(|a, b| a < b),
            "ICONS must be sorted, each once"
        );
        for icon in ICONS {
            assert!(icon.starts_with("fa-"), "{icon} without its fa- prefix");
            assert_eq!(normalize(icon), Some(*icon));
        }
    }

    #[test]
    fn the_style_class_and_blanks_are_dropped() {
        assert_eq!(normalize(" fa-solid fa-cow "), Some("fa-cow"));
        assert_eq!(normalize("fa-cow"), Some("fa-cow"));
        assert_eq!(normalize("  "), None);
        assert_eq!(normalize("fa-solid "), None);
        assert_eq!(known("fa-cow"), Some("fa-cow"));
        assert_eq!(known("Cow"), Some("fa-cow"));
        assert_eq!(known("fa-sedling"), None);
    }

    #[test]
    fn a_search_puts_the_name_and_its_starts_first() {
        let found = search("sun", 5);
        assert_eq!(found.first(), Some(&"fa-sun"));
        assert!(found.iter().all(|i| i.contains("sun")));
        assert_eq!(search("cloud rain", 3), search("fa-cloud-rain", 3));
        assert_eq!(search("cloud rain", 1), vec!["fa-cloud-rain"]);
        assert!(search("rain cloud", 50).contains(&"fa-cloud-rain"));
        assert_eq!(search("", 2), ICONS.get(..2).unwrap_or_default());
        assert!(search("zzzz", 5).is_empty());
    }
}
