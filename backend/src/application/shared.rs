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
            "Tên đăng nhập phải có ít nhất {USERNAME_MIN_LEN} ký tự."
        )));
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < PASSWORD_MIN_LEN {
        return Err(AppError::BadRequest(format!(
            "Mật khẩu phải có ít nhất {PASSWORD_MIN_LEN} ký tự."
        )));
    }
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err(AppError::BadRequest(
            "Mật khẩu phải chứa ít nhất một chữ hoa (A–Z).".into(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(
            "Mật khẩu phải chứa ít nhất một chữ số (0–9).".into(),
        ));
    }
    if !password.chars().any(|c| !c.is_alphanumeric()) {
        return Err(AppError::BadRequest(
            "Mật khẩu phải chứa ít nhất một ký tự đặc biệt (ví dụ: @, #, !).".into(),
        ));
    }
    Ok(())
}

pub fn validate_coordinates(lat: f64, lng: f64) -> Result<(), AppError> {
    if !(LAT_MIN..=LAT_MAX).contains(&lat) || !(LNG_MIN..=LNG_MAX).contains(&lng) {
        return Err(AppError::BadRequest(
            "Toạ độ không hợp lệ. lat phải trong [-90, 90], lng trong [-180, 180].".into(),
        ));
    }
    Ok(())
}

pub fn validate_phone(phone: &str) -> Result<(), AppError> {
    if !is_valid_vn_phone(phone) {
        return Err(AppError::BadRequest(
            "Số điện thoại không hợp lệ (ví dụ: 0901234567).".into(),
        ));
    }
    Ok(())
}
