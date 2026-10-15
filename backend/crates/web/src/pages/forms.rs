//! What every page form needs when it comes back with a problem: one
//! localised message per field, and one for the form as a whole.

use std::collections::BTreeMap;

use domain::{Error, ErrorCode, ErrorKind, Validation};
use serde::Serialize;

use crate::{app_state::AppState, error_response::error_body};

#[derive(Debug, Default, Serialize)]
pub struct FormErrors {
    /// Field name → message, shown under that field.
    pub fields: BTreeMap<&'static str, String>,
    /// Shown above the fields.
    pub message: Option<String>,
}

impl FormErrors {
    /// The localised texts of `err`: its message, and the first message of
    /// each field it names.
    pub fn from_error(state: &AppState, err: &Error) -> Self {
        let body = error_body(state, err);
        Self {
            fields: body
                .field_errors
                .into_iter()
                .filter_map(|(field, mut messages)| {
                    (!messages.is_empty()).then(|| (field, messages.swap_remove(0)))
                })
                .collect(),
            message: Some(body.message),
        }
    }
}

/// A form error the visitor can fix by editing the form (a 4xx that is not
/// about the session): the page is re-shown with it rather than replaced by
/// the error page.
pub fn is_form_error(err: &Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::BadRequest
            | ErrorKind::Unauthorized
            | ErrorKind::Conflict
            | ErrorKind::Locked
            | ErrorKind::Forbidden
    )
}

/// The "type it again" box must match. Checked before the use case runs, so
/// a typo never creates an account.
pub fn confirm_password(
    password: &str,
    confirmation: &str,
    field: &'static str,
) -> Result<(), Error> {
    Validation::new()
        .check(password != confirmation, field, ErrorCode::PasswordMismatch)
        .finish()
}

/// An optional "why" box for the audit log: blank means no reason.
pub fn reason(raw: &str) -> Option<String> {
    Some(raw.trim().to_owned()).filter(|r| !r.is_empty())
}

/// A whole-number box: blank is the type's default (0), anything else must
/// parse, or it is a field error on `field`.
pub fn whole_number<T: std::str::FromStr + Default>(
    raw: &str,
    field: &'static str,
) -> Result<T, Error> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(T::default());
    }
    raw.parse()
        .map_err(|_| Error::field(field, ErrorCode::InvalidNumber))
}

/// A latitude or longitude box. Text that is not a number becomes NaN, which
/// the use case's coordinate rule rejects with its own field error, so the
/// form still gets every other message in the same round trip. A comma is
/// accepted as the decimal mark, as Vietnamese keyboards type it.
pub fn coordinate(raw: &str) -> f64 {
    raw.trim().replace(',', ".").parse().unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_boxes_parse_leniently_but_never_guess() {
        assert_eq!(whole_number::<i16>("  ", "display_order").ok(), Some(0));
        assert_eq!(whole_number::<i16>(" 7 ", "display_order").ok(), Some(7));
        let err = whole_number::<i16>("99999", "display_order").err();
        assert_eq!(
            err.as_ref()
                .and_then(|e| e.fields().first().map(|f| (f.field, f.code))),
            Some(("display_order", ErrorCode::InvalidNumber))
        );
        assert_eq!(coordinate("10,36"), 10.36);
        assert!(coordinate("mười").is_nan());
        assert_eq!(reason("  "), None);
        assert_eq!(reason(" giả "), Some("giả".to_owned()));
    }

    #[test]
    fn a_mismatch_is_a_field_error_on_the_confirmation() {
        let err = confirm_password("matkhau123", "matkhau12", "password2").err();
        assert_eq!(
            err.as_ref().map(Error::code),
            Some(ErrorCode::ValidationFailed)
        );
        assert_eq!(
            err.as_ref()
                .and_then(|e| e.fields().first().map(|f| (f.field, f.code))),
            Some(("password2", ErrorCode::PasswordMismatch))
        );
        assert!(confirm_password("a", "a", "password2").is_ok());
    }

    #[test]
    fn server_failures_are_not_form_errors() {
        assert!(is_form_error(&Error::new(ErrorCode::InvalidCredentials)));
        assert!(is_form_error(&Error::new(ErrorCode::AccountLocked)));
        assert!(!is_form_error(&Error::internal("db down")));
        assert!(!is_form_error(&Error::new(ErrorCode::DatabaseUnavailable)));
    }
}
