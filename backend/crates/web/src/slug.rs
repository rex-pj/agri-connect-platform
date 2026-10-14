//! URL slugs, identical to the SvelteKit `lib/slugify.ts` so links built on
//! either side reach the same canonical URL while both run.

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
mod tests {
    use super::slugify;

    #[test]
    fn matches_the_sveltekit_slugs() {
        // Each pair is what slugify.ts returns for the same input.
        for (input, expected) in [
            ("Gạo Thơm ST25 An Giang", "gao-thom-st25-an-giang"),
            ("Đại lý Phân bón Đồng Tháp", "dai-ly-phan-bon-dong-thap"),
            ("Sầu riêng Ri6 – loại 1", "sau-rieng-ri6-loai-1"),
            ("  Dịch vụ   Múc bùn  ", "dich-vu-muc-bun"),
            ("Phân NPK 20-20-15 (bao 50kg)", "phan-npk-20-20-15-bao-50kg"),
            ("a -- b", "a-b"),
            ("!!!", ""),
        ] {
            assert_eq!(slugify(input), expected, "{input}");
        }
    }
}
