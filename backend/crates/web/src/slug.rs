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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::slugify;

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
