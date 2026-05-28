/// Boolean predicate — true when `phone` is a valid 10-digit Vietnamese mobile number.
pub fn is_valid_vn_phone(phone: &str) -> bool {
    phone.len() == 10
        && phone.starts_with('0')
        && phone.chars().all(|c| c.is_ascii_digit())
}

/// `validator`-crate integration — use as `#[validate(custom(function = "validate_vn_phone"))]`.
pub fn validate_vn_phone(phone: &str) -> Result<(), validator::ValidationError> {
    if is_valid_vn_phone(phone) {
        Ok(())
    } else {
        let mut err = validator::ValidationError::new("invalid_phone");
        err.message = Some("Invalid phone number (10 digits, starting with 0)".into());
        Err(err)
    }
}
