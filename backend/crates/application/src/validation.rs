//! Field rules shared by use cases. Each returns a coded `Error`; callers that
//! validate a whole form feed them into `domain::Validation`.

use domain::{Error, ErrorCode, Result};

use crate::constants::{
    LAT_MAX, LAT_MIN, LNG_MAX, LNG_MIN, PASSWORD_MAX_BYTES, PASSWORD_MIN_CHARS, USERNAME_MIN_LEN,
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
pub fn non_blank(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

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
            validate_password(&"a".repeat(PASSWORD_MAX_BYTES + 1)).err().map(|e| e.code()),
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
