//! Field rules shared by use cases. Each returns a coded `Error`; callers that
//! validate a whole form feed them into `domain::Validation`.

use domain::{Error, ErrorCode, Result};

use crate::constants::{
    EMAIL_MAX, LAT_MAX, LAT_MIN, LNG_MAX, LNG_MIN, PASSWORD_MAX_BYTES, PASSWORD_MIN_CHARS,
    USERNAME_MIN_LEN,
};

/// 10-digit Vietnamese mobile number starting with 0. Mirrors the
/// `chk_*_phone` CHECK constraints in the baseline migration.
pub fn is_valid_vn_phone(phone: &str) -> bool {
    phone.len() == 10 && phone.starts_with('0') && phone.chars().all(|c| c.is_ascii_digit())
}

pub fn validate_phone(phone: &str) -> Result<()> {
    if is_valid_vn_phone(phone) {
        Ok(())
    } else {
        Err(Error::new(ErrorCode::InvalidPhone))
    }
}

pub fn validate_username(username: &str) -> Result<()> {
    if username.chars().count() < USERNAME_MIN_LEN {
        return Err(Error::new(ErrorCode::UsernameTooShort).with_param("min", USERNAME_MIN_LEN));
    }
    Ok(())
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
