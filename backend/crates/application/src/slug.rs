//! URL slugs. They are part of every canonical product, provider and
//! category URL, so changing the output moves pages: the fixtures pin it.

use unicode_normalization::UnicodeNormalization;

/// "Gạo Thơm ST25 An Giang" → "gao-thom-st25-an-giang". `đ`/`Đ` are letters of
/// their own, not `d` plus an accent, so NFD leaves them alone: mapped first.
pub fn slugify(text: &str) -> String {
    let folded: String = text
        .chars()
        .map(|c| if matches!(c, 'đ' | 'Đ') { 'd' } else { c })
        .collect::<String>()
        .nfd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect::<String>()
        .to_lowercase();
    let kept: String = folded
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c.is_whitespace() || *c == '-')
        .collect();
    // Whitespace runs become one hyphen, then hyphen runs collapse.
    let hyphenated = kept.split_whitespace().collect::<Vec<_>>().join("-");
    let mut out = String::with_capacity(hyphenated.len());
    for c in hyphenated.chars() {
        if c == '-' && out.ends_with('-') {
            continue;
        }
        out.push(c);
    }
    out
}

/// The `n`-th candidate for a free slug: `base` for 1, then `base-2`,
/// `base-3`, …, cut to `max` characters with the suffix kept whole (and no
/// hyphen left dangling before it).
pub fn numbered(base: &str, n: u32, max: usize) -> String {
    let suffix = if n <= 1 {
        String::new()
    } else {
        format!("-{n}")
    };
    let room = max.saturating_sub(suffix.len());
    let stem: String = base.chars().take(room).collect();
    format!("{}{suffix}", stem.trim_end_matches('-'))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{numbered, slugify};

    #[test]
    fn numbered_slugs_keep_their_suffix_within_the_limit() {
        assert_eq!(numbered("bom-nuoc", 1, 100), "bom-nuoc");
        assert_eq!(numbered("bom-nuoc", 2, 100), "bom-nuoc-2");
        assert_eq!(numbered("bom-nuoc", 12, 10), "bom-nuo-12");
        // The cut lands on a hyphen: it is dropped, not doubled.
        assert_eq!(numbered("bom-nuoc", 2, 6), "bom-2");
        assert_eq!(numbered("bom-nuoc-tot", 1, 8), "bom-nuoc");
    }

    /// The slugs published URLs already use.
    const FIXTURES: &str = include_str!("slugify.fixtures.json");

    #[test]
    fn slugs_stay_what_published_urls_use() {
        let fixtures: Vec<(String, String)> = serde_json::from_str(FIXTURES)
            .expect("slugify.fixtures.json is a list of [input, slug]");
        assert!(fixtures.len() >= 5, "fixtures went missing");
        for (input, expected) in fixtures {
            assert_eq!(slugify(&input), expected, "{input:?}");
        }
    }
}
