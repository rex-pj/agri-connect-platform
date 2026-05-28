//! Application-layer validation helpers.
//!
//! All functions here take plain Rust types and return `AppError` so use
//! cases can call them directly. No HTTP or infrastructure types are used.
//!
//! Password hashing/verification is NOT here — those are infrastructure
//! concerns injected via the `PasswordHasher` port in `application/ports`.

use crate::{
    constants::{
        LAT_MAX, LAT_MIN, LNG_MAX, LNG_MIN, PASSWORD_MIN_LEN, ROLE_ADMIN, ROLE_PROVIDER,
        USERNAME_MIN_LEN,
    },
    validators::is_valid_vn_phone,
    view_models::AppError,
};

// ── Identity validation ───────────────────────────────────────────────────────

pub fn validate_username(username: &str) -> Result<(), AppError> {
    if username.len() < USERNAME_MIN_LEN {
        return Err(AppError::BadRequest(format!(
            "Username must be at least {USERNAME_MIN_LEN} characters."
        )));
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < PASSWORD_MIN_LEN {
        return Err(AppError::BadRequest(format!(
            "Password must be at least {PASSWORD_MIN_LEN} characters."
        )));
    }
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err(AppError::BadRequest(
            "Password must contain at least one uppercase letter (A–Z).".into(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(
            "Password must contain at least one digit (0–9).".into(),
        ));
    }
    if !password.chars().any(|c| !c.is_alphanumeric()) {
        return Err(AppError::BadRequest(
            "Password must contain at least one special character (e.g. @, #, !).".into(),
        ));
    }
    Ok(())
}

// ── Geo validation ────────────────────────────────────────────────────────────

pub fn validate_coordinates(lat: f64, lng: f64) -> Result<(), AppError> {
    if !(LAT_MIN..=LAT_MAX).contains(&lat) || !(LNG_MIN..=LNG_MAX).contains(&lng) {
        return Err(AppError::BadRequest(
            "Invalid coordinates. lat must be in [-90, 90], lng in [-180, 180].".into(),
        ));
    }
    Ok(())
}

// ── Contact validation ────────────────────────────────────────────────────────

pub fn validate_phone(phone: &str) -> Result<(), AppError> {
    if !is_valid_vn_phone(phone) {
        return Err(AppError::BadRequest(
            "Invalid phone number (e.g. 0901234567).".into(),
        ));
    }
    Ok(())
}

// ── Business entity validation ────────────────────────────────────────────────

/// Validate a (name, slug) pair used by categories and similar entities.
/// Both fields must be non-empty after trimming.
pub fn validate_name_and_slug(name: &str, slug: &str) -> Result<(), AppError> {
    if name.is_empty() || slug.is_empty() {
        return Err(AppError::BadRequest(
            "Name and slug must not be empty.".into(),
        ));
    }
    Ok(())
}

/// Validate a product price: if supplied, it must be a positive integer.
pub fn validate_price(price: Option<i64>) -> Result<(), AppError> {
    if let Some(p) = price {
        if p <= 0 {
            return Err(AppError::BadRequest(
                "Product price must be greater than 0.".into(),
            ));
        }
    }
    Ok(())
}

// ── Role code validation ──────────────────────────────────────────────────────

/// Validate that a role code is one of the recognised application roles.
/// Returns the valid code on success so callers can use it directly.
pub fn validate_role_code(role_code: &str) -> Result<&str, AppError> {
    let trimmed = role_code.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("Role code must not be empty.".into()));
    }
    if trimmed != ROLE_ADMIN && trimmed != ROLE_PROVIDER {
        return Err(AppError::BadRequest(format!(
            "Invalid role code. Must be one of: {ROLE_ADMIN}, {ROLE_PROVIDER}."
        )));
    }
    Ok(trimmed)
}
