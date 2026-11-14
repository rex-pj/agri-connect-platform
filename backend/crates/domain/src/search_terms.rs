//! Words farmers use for the same thing, so a search finds it under either:
//! the South's word and the North's (heo, lợn), the trade's and the
//! farmer's (sên vét, nạo vét). Kept to pairs that never mean anything else
//! inside another word; the repositories search every variant of a query.
//!
//! These are search vocabulary, not wording shown to anyone, which is why
//! they live here and not in the locale files.

/// Each group: words that name the same thing, lowercase, with diacritics.
pub const SYNONYMS: &[&[&str]] = &[
    &["heo", "lợn"],
    &["bắp", "ngô"],
    &["tôm thẻ", "tôm chân trắng"],
    &["phun thuốc", "xịt thuốc"],
    &["sên vét", "nạo vét"],
    &["cám", "thức ăn"],
    &["máy cày", "máy kéo"],
];

/// `query` and the same query with each synonym it contains swapped for the
/// others of its group; the query itself first, no repeats.
pub fn query_variants(query: &str) -> Vec<String> {
    let query = query.trim().to_lowercase();
    let mut variants = vec![query.clone()];
    for group in SYNONYMS {
        for word in *group {
            if !query.contains(word) {
                continue;
            }
            for other in group.iter().filter(|o| *o != word) {
                let variant = query.replace(word, other);
                if !variants.contains(&variant) {
                    variants.push(variant);
                }
            }
        }
    }
    variants
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_of_a_group_brings_the_others() {
        assert_eq!(query_variants("Heo hơi"), ["heo hơi", "lợn hơi"]);
        assert_eq!(
            query_variants("cám lợn"),
            ["cám lợn", "cám heo", "thức ăn lợn"]
        );
        assert_eq!(query_variants("múc bùn"), ["múc bùn"]);
    }

    #[test]
    fn every_group_has_two_lowercase_words_or_more() {
        for group in SYNONYMS {
            assert!(group.len() >= 2, "{group:?}");
            for w in *group {
                assert_eq!(*w, w.to_lowercase(), "{w}");
            }
        }
    }
}
