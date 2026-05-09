use std::collections::HashMap;
use validator::{ValidationError, ValidationErrors};

/// Boolean check — true when `phone` is a valid 10-digit Vietnamese mobile number.
pub fn is_valid_vn_phone(phone: &str) -> bool {
    phone.len() == 10
        && phone.starts_with('0')
        && phone.chars().all(|c| c.is_ascii_digit())
}

/// validator-crate integration — use as `#[validate(custom(function = "validate_vn_phone"))]`.
pub fn validate_vn_phone(phone: &str) -> Result<(), ValidationError> {
    if is_valid_vn_phone(phone) {
        Ok(())
    } else {
        let mut err = ValidationError::new("invalid_phone");
        err.message = Some(
            "Số điện thoại không hợp lệ (10 chữ số, bắt đầu bằng 0)".into(),
        );
        Err(err)
    }
}

pub struct ValidationHelper;

impl ValidationHelper {
    pub fn flatten_errors(e: ValidationErrors) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for (field, errors) in e.field_errors() {
            let msg = errors
                .first()
                .and_then(|e| e.message.as_ref())
                .map(|m| m.to_string())
                .unwrap_or_else(|| format!("Giá trị không hợp lệ: {}", field));
            map.insert(field.to_string(), msg);
        }
        map
    }
}
