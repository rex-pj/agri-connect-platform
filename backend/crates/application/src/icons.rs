//! The icons an admin may give a sector, an alert kind, a commodity, a
//! service or supply type: every name the vendored Font Awesome 6 defines
//! (aliases too), offered by the picker (`molecules/form_field.html`
//! `icon_select`) under that name. A name is stored as `fa-…`;
//! `atoms/icon.html` adds the style.

/// The style class an older form stored before the name (`fa-solid fa-cow`):
/// accepted and dropped, so such a row saves unchanged.
const STYLE_CLASS: &str = "fa-solid";

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

/// Whether `name` (normalized) is one of the picker's.
pub fn is_known(name: &str) -> bool {
    ICONS.binary_search(&name).is_ok()
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
        assert!(is_known("fa-cow"));
        assert!(!is_known("fa-sedling"));
    }
}
