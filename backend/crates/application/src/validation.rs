//! Field rules shared by use cases. Each returns a coded `Error`; callers that
//! validate a whole form feed them into `domain::Validation`.

use domain::{models::coded::CodedEntry, Error, ErrorCode, Result};

use crate::constants::{
    AUDIT_REASON_MAX, EMAIL_MAX, LAT_MAX, LAT_MIN, LNG_MAX, LNG_MIN, PASSWORD_MAX_BYTES,
    PASSWORD_MIN_CHARS, SEARCH_QUERY_MIN, USERNAME_MAX_LEN, USERNAME_MIN_LEN,
};

/// The optional "why" an admin gives for a change, as the audit log keeps it:
/// trimmed, blank as none, at most `AUDIT_REASON_MAX` characters. The forms
/// stop typing at that length, so only a hand-made request is cut.
pub fn audit_reason(reason: Option<String>) -> Option<String> {
    let reason = reason?;
    let reason = reason.trim();
    (!reason.is_empty()).then(|| reason.chars().take(AUDIT_REASON_MAX).collect())
}

/// A code as sent, one of `known` (the entries stored: the admin forms take
/// a hidden one too, the public choices only the shown ones): `None` for
/// blank, `invalid` for any other.
pub fn coded(text: &str, known: &[CodedEntry], invalid: ErrorCode) -> Result<Option<String>> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    known
        .iter()
        .find(|s| s.code == text)
        .map(|s| Some(s.code.clone()))
        .ok_or_else(|| Error::new(invalid))
}

/// A sector code as sent (`coded`): `None` for blank (every sector),
/// `invalid_sector` for one not in `known`.
pub fn sector(text: &str, known: &[CodedEntry]) -> Result<Option<String>> {
    coded(text, known, ErrorCode::InvalidSector)
}

/// Digits of a Vietnamese mobile number as kept: 0 and nine more.
pub const VN_PHONE_DIGITS: usize = 10;
/// Vietnam's calling code, as an international number starts.
pub const VN_CALLING_CODE: &str = "84";

/// `is_valid_vn_phone` as an HTML `pattern`: the forms stop a wrong number
/// before it is sent (the rule above still decides).
pub const VN_PHONE_PATTERN: &str = "0[0-9]{9}";

/// `validate_slug` as an HTML `pattern`.
pub const SLUG_PATTERN: &str = "[a-z0-9]+(-[a-z0-9]+)*";

/// 10-digit Vietnamese mobile number starting with 0. Mirrors the
/// `chk_*_phone` CHECK constraints in the baseline migration.
pub fn is_valid_vn_phone(phone: &str) -> bool {
    phone.len() == VN_PHONE_DIGITS
        && phone.starts_with('0')
        && phone.chars().all(|c| c.is_ascii_digit())
}

pub fn validate_phone(phone: &str) -> Result<()> {
    if is_valid_vn_phone(phone) {
        Ok(())
    } else {
        Err(Error::new(ErrorCode::InvalidPhone))
    }
}

/// A login name: at least `USERNAME_MIN_LEN` and at most `USERNAME_MAX_LEN`
/// characters, no spaces (callers pass it trimmed and lowercased). Letters
/// with Vietnamese marks are fine.
pub fn validate_username(username: &str) -> Result<()> {
    let length = username.chars().count();
    if length < USERNAME_MIN_LEN {
        return Err(Error::new(ErrorCode::UsernameTooShort).with_param("min", USERNAME_MIN_LEN));
    }
    if username
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(Error::new(ErrorCode::UsernameHasSpaces));
    }
    max_chars(username, USERNAME_MAX_LEN)
}

/// An address mail can be sent to: `local@domain.tld`, no spaces, one `@`,
/// a dotted domain without empty labels. Callers pass it trimmed and
/// lowercased. Deliverability is not checked: the reset email is the test.
pub fn validate_email(email: &str) -> Result<()> {
    let valid = email.chars().count() <= EMAIL_MAX
        && !email.chars().any(char::is_whitespace)
        && match email.split_once('@') {
            Some((local, domain)) => {
                !local.is_empty()
                    && !domain.contains('@')
                    && domain.contains('.')
                    && domain.split('.').all(|label| !label.is_empty())
            }
            None => false,
        };
    if valid {
        Ok(())
    } else {
        Err(Error::new(ErrorCode::InvalidEmail))
    }
}

pub fn validate_password(password: &str) -> Result<()> {
    if password.chars().count() < PASSWORD_MIN_CHARS {
        return Err(Error::new(ErrorCode::PasswordTooShort).with_param("min", PASSWORD_MIN_CHARS));
    }
    if password.len() > PASSWORD_MAX_BYTES {
        return Err(Error::new(ErrorCode::PasswordTooLong).with_param("max", PASSWORD_MAX_BYTES));
    }
    Ok(())
}

pub fn validate_coordinates(lat: f64, lng: f64) -> Result<()> {
    if (LAT_MIN..=LAT_MAX).contains(&lat) && (LNG_MIN..=LNG_MAX).contains(&lng) {
        Ok(())
    } else {
        Err(Error::new(ErrorCode::InvalidCoordinates))
    }
}

/// The PostGIS point for a validated position: SRID 4326, longitude first.
pub fn point_wkt(lat: f64, lng: f64) -> String {
    format!("SRID=4326;POINT({lng} {lat})")
}

/// A product price, when given, must be a positive number of đồng.
pub fn validate_price(price: Option<i64>) -> Result<()> {
    match price {
        Some(p) if p <= 0 => Err(Error::new(ErrorCode::InvalidPrice)),
        _ => Ok(()),
    }
}

/// Trims and maps blank input to `None`, so optional text fields never store "".
/// A URL slug: lowercase ASCII letters and digits in hyphen-separated words.
/// Anything else would make a category URL that needs escaping or breaks.
pub fn validate_slug(slug: &str) -> Result<()> {
    let ok = !slug.is_empty()
        && slug.split('-').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        });
    if ok {
        Ok(())
    } else {
        Err(Error::new(ErrorCode::InvalidSlug))
    }
}

/// At most `max` characters (not bytes): the database column's size, so an
/// overlong value is a field message instead of a failed INSERT.
pub fn max_chars(value: &str, max: usize) -> Result<()> {
    if value.chars().count() <= max {
        Ok(())
    } else {
        Err(Error::new(ErrorCode::FieldTooLong).with_param("max", max))
    }
}

/// `max_chars` for an optional box: nothing typed is never too long.
pub fn max_chars_opt(value: &Option<String>, max: usize) -> Result<()> {
    value.as_deref().map_or(Ok(()), |v| max_chars(v, max))
}

pub fn non_blank(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// The words typed in a public search box: trimmed, blank as none, and at
/// least `SEARCH_QUERY_MIN` characters. A single letter is in nearly every
/// name, so it finds nothing a farmer could use, and no index can answer it:
/// the database would compare every row.
pub fn search_text(typed: Option<String>) -> Result<Option<String>> {
    match non_blank(typed) {
        Some(words) if words.chars().count() < SEARCH_QUERY_MIN => {
            Err(Error::new(ErrorCode::SearchTooShort).with_param("min", SEARCH_QUERY_MIN))
        }
        words => Ok(words),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_need_a_local_part_and_a_dotted_domain() {
        for ok in ["nongdan@gmail.com", "a.b+c@htx.cailay.vn"] {
            assert!(validate_email(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "nongdan",
            "@gmail.com",
            "nongdan@gmail",
            "nong dan@gmail.com",
            "a@b@c.vn",
            "a@.vn",
            "a@gmail..com",
        ] {
            assert!(validate_email(bad).is_err(), "{bad:?}");
        }
        let long = format!("{}@x.vn", "a".repeat(EMAIL_MAX));
        assert!(validate_email(&long).is_err());
    }

    #[test]
    fn usernames_are_bounded_and_have_no_spaces() {
        assert!(validate_username("nông_dân.7").is_ok());
        let code = |u: &str| validate_username(u).err().map(|e| e.code());
        assert_eq!(code("ab"), Some(ErrorCode::UsernameTooShort));
        assert_eq!(code("nong dan"), Some(ErrorCode::UsernameHasSpaces));
        assert_eq!(
            code(&"a".repeat(USERNAME_MAX_LEN + 1)),
            Some(ErrorCode::FieldTooLong)
        );
    }

    #[test]
    fn slugs_are_lowercase_ascii_words() {
        assert!(validate_slug("phan-bon").is_ok());
        assert!(validate_slug("npk20").is_ok());
        for bad in [
            "",
            "Phan-bon",
            "phân-bón",
            "phan bon",
            "-phan",
            "phan--bon",
            "phan/bon",
        ] {
            assert!(validate_slug(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn lengths_count_characters() {
        assert!(max_chars("Phân bón", 8).is_ok());
        assert!(max_chars("Phân bón!", 8).is_err());
    }

    #[test]
    fn phone_rule_matches_check_constraint() {
        assert!(is_valid_vn_phone("0901234567"));
        assert!(!is_valid_vn_phone("901234567"));
        assert!(!is_valid_vn_phone("1901234567"));
        assert!(!is_valid_vn_phone("09012345a7"));
        assert!(!is_valid_vn_phone("09012345678"));
    }

    #[test]
    fn password_counts_characters_not_bytes() {
        // 8 Vietnamese letters are 8 characters but more than 8 bytes.
        assert!(validate_password("mậtkhẩuư").is_ok());
        assert_eq!(
            validate_password("1234567").err().map(|e| e.code()),
            Some(ErrorCode::PasswordTooShort)
        );
    }

    #[test]
    fn password_over_bcrypt_limit_is_rejected() {
        assert!(validate_password(&"a".repeat(PASSWORD_MAX_BYTES)).is_ok());
        assert_eq!(
            validate_password(&"a".repeat(PASSWORD_MAX_BYTES + 1))
                .err()
                .map(|e| e.code()),
            Some(ErrorCode::PasswordTooLong)
        );
    }

    #[test]
    fn non_blank_trims_and_drops_empty() {
        assert_eq!(non_blank(Some("  x ".into())), Some("x".into()));
        assert_eq!(non_blank(Some("   ".into())), None);
        assert_eq!(non_blank(None), None);
    }
}
