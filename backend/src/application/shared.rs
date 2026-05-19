use tracing::error;

use crate::{
    constants::{
        LAT_MAX, LAT_MIN, LNG_MAX, LNG_MIN, PASSWORD_MIN_LEN, USERNAME_MIN_LEN,
    },
    validators::is_valid_vn_phone,
    view_models::AppError,
};

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

pub fn validate_coordinates(lat: f64, lng: f64) -> Result<(), AppError> {
    if !(LAT_MIN..=LAT_MAX).contains(&lat) || !(LNG_MIN..=LNG_MAX).contains(&lng) {
        return Err(AppError::BadRequest(
            "Invalid coordinates. lat must be in [-90, 90], lng in [-180, 180].".into(),
        ));
    }
    Ok(())
}

pub fn validate_phone(phone: &str) -> Result<(), AppError> {
    if !is_valid_vn_phone(phone) {
        return Err(AppError::BadRequest(
            "Invalid phone number (e.g. 0901234567).".into(),
        ));
    }
    Ok(())
}

/// Hash a password using bcrypt. Offloaded to a blocking thread pool so the
/// Tokio worker thread is never stalled by this CPU-intensive KDF.
pub async fn bcrypt_hash(password: &str) -> Result<String, AppError> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || bcrypt::hash(&password, bcrypt::DEFAULT_COST))
        .await
        .map_err(|e| {
            error!("bcrypt hash join error: {e}");
            AppError::Internal("Password hashing error.".into())
        })?
        .map_err(|e| {
            error!("bcrypt hash error: {e}");
            AppError::Internal("Password hashing error.".into())
        })
}

/// Verify a bcrypt hash. Offloaded to a blocking thread pool for the same
/// reason as `bcrypt_hash`.
pub async fn bcrypt_verify(password: &str, hash: &str) -> Result<bool, AppError> {
    let password = password.to_owned();
    let hash     = hash.to_owned();
    tokio::task::spawn_blocking(move || bcrypt::verify(&password, &hash))
        .await
        .map_err(|e| {
            error!("bcrypt verify join error: {e}");
            AppError::Internal("Authentication error.".into())
        })?
        .map_err(|e| {
            error!("bcrypt verify error: {e}");
            AppError::Internal("Authentication error.".into())
        })
}
