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

/// The Vietnamese alphabet, in dictionary order: ă, â, đ, ê, ô, ơ, ư are
/// letters of their own, after the one they are written on.
const ALPHABET: &str = "aăâbcdđeêfghijklmnoôơpqrstuưvwxyz";

/// Combining marks that make a letter of its own: breve (ă), circumflex
/// (â, ê, ô), horn (ơ, ư).
const LETTER_MARKS: [char; 3] = ['\u{0306}', '\u{0302}', '\u{031b}'];

/// Tone marks, in dictionary order after the level tone: grave, acute,
/// hook, tilde, dot below.
const TONES: [char; 5] = ['\u{0300}', '\u{0301}', '\u{0309}', '\u{0303}', '\u{0323}'];

/// A key that sorts names in Vietnamese dictionary order: "Cà Mau", "Cao
/// Bằng", "Cần Thơ", then every D before "Đà Nẵng". Case is ignored; tones
/// only break ties between names spelt with the same letters. A space sorts
/// before any letter, digits before letters, anything else after them.
pub fn sort_key(text: &str) -> (Vec<u32>, Vec<u8>) {
    let alphabet: Vec<char> = ALPHABET.chars().collect();
    let mut letters = Vec::new();
    let mut tones = Vec::new();
    for c in text.to_lowercase().chars() {
        let mut base = None;
        let mut tone = 0;
        for part in c.nfd() {
            if let Some(i) = TONES.iter().position(|t| *t == part) {
                tone = i + 1;
            } else if LETTER_MARKS.contains(&part) {
                base = base.map(|b: String| format!("{b}{part}"));
            } else {
                base = Some(part.to_string());
            }
        }
        let letter: String = base.unwrap_or_default().nfc().collect();
        let mut chars = letter.chars();
        let weight = match (chars.next(), chars.next()) {
            (Some(' '), None) => 0,
            (Some(d), None) if d.is_ascii_digit() => 1 + u32::from(d) - u32::from('0'),
            (Some(l), None) => alphabet
                .iter()
                .position(|a| *a == l)
                .map_or(1_000 + u32::from(l), |i| 100 + i as u32),
            _ => 1_000,
        };
        letters.push(weight);
        tones.push(tone as u8);
    }
    (letters, tones)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{numbered, slugify, sort_key};

    #[test]
    fn names_sort_in_vietnamese_dictionary_order() {
        let mut names = vec![
            "Cần Thơ",
            "Đà Nẵng",
            "Cao Bằng",
            "Hải Phòng",
            "Cà Mau",
            "Hà Nội",
            "An Giang",
            "Bắc Ninh",
            "Điện Biên",
            "Dĩ An",
            "ca mau",
        ];
        names.sort_by_key(|n| sort_key(n));
        assert_eq!(
            names,
            vec![
                "An Giang",
                "Bắc Ninh",
                "ca mau",
                "Cà Mau",
                "Cao Bằng",
                "Cần Thơ",
                "Dĩ An",
                "Đà Nẵng",
                "Điện Biên",
                "Hà Nội",
                "Hải Phòng",
            ],
            "a < ă < â, d < đ, a space before a letter, the tone only after"
        );
    }

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
