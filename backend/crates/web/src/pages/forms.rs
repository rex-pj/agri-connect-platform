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

    /// `err` shown under `field` rather than above the form: for a refusal
    /// that is about one box but is not a rule of the form (a photo that is
    /// too big or not an image).
    pub fn on_field(state: &AppState, err: &Error, field: &'static str) -> Self {
        let body = error_body(state, err);
        Self {
            fields: BTreeMap::from([(field, body.message)]),
            message: None,
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
    application::validation::audit_reason(Some(raw.to_owned()))
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

/// A price box in đồng: blank is no price. Dots, commas and spaces are
/// thousands separators ("500.000"), since đồng have no decimals; anything
/// else that is not a whole number is a field error on `field`.
pub fn vnd_amount(raw: &str, field: &'static str) -> Result<Option<i64>, Error> {
    let digits: String = raw
        .chars()
        .filter(|c| !matches!(c, '.' | ',' | ' '))
        .collect();
    if digits.is_empty() {
        return Ok(None);
    }
    digits
        .parse()
        .map(Some)
        .map_err(|_| Error::field(field, ErrorCode::InvalidNumber))
}

/// An optional coordinate box: blank is none, anything else as `coordinate`.
pub fn optional_coordinate(raw: &str) -> Option<f64> {
    (!raw.trim().is_empty()).then(|| coordinate(raw))
}

/// A latitude or longitude box. Text that is not a number becomes NaN, which
/// the use case's coordinate rule rejects with its own field error, so the
/// form still gets every other message in the same round trip. A comma is
/// accepted as the decimal mark, as Vietnamese keyboards type it.
pub fn coordinate(raw: &str) -> f64 {
    raw.trim().replace(',', ".").parse().unwrap_or(f64::NAN)
}

/// What a point box holds: nothing, a point (latitude, longitude), or text
/// that is neither (a short link, a place name).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypedPoint {
    Empty,
    Point(f64, f64),
    Unreadable,
}

/// A point as people paste it: "10.4048, 106.1180" (what Google Maps
/// copies), "10,4048 106,118" (comma decimals, as Vietnamese keyboards type
/// them), or a Google Maps link: a place's own pin (`!3d10.4!4d106.1`)
/// before the map's centre (`@10.4,106.1,17z`), or `?q=` / `?query=` /
/// `?ll=`. A pair whose first number cannot be a latitude was written the
/// other way round and is turned. A short link (maps.app.goo.gl) holds no
/// point: it is unreadable. `static/js/app.js` `typedPoint` reads the same.
pub fn typed_point(raw: &str) -> TypedPoint {
    let text = raw.trim();
    if text.is_empty() {
        return TypedPoint::Empty;
    }
    match link_point(text).or_else(|| plain_point(text)) {
        Some((a, b)) if a.abs() > 90.0 && b.abs() <= 90.0 => TypedPoint::Point(b, a),
        Some((a, b)) => TypedPoint::Point(a, b),
        None => TypedPoint::Unreadable,
    }
}

/// The text after the first `key`.
fn after<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.find(key).map(|at| &text[at + key.len()..])
}

/// The number `text` starts with (`-10.4048`), and the rest.
fn leading_number(text: &str) -> Option<(f64, &str)> {
    let end = text
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || (i == 0 && c == '-')))
        .map_or(text.len(), |(i, _)| i);
    let value: f64 = text[..end].parse().ok()?;
    value.is_finite().then_some((value, &text[end..]))
}

/// "10.4,106.1" (or `%2C`) at the start of `text`.
fn leading_pair(text: &str) -> Option<(f64, f64)> {
    let (lat, rest) = leading_number(text)?;
    let rest = rest
        .strip_prefix(',')
        .or_else(|| rest.strip_prefix("%2C"))
        .or_else(|| rest.strip_prefix("%2c"))?;
    let rest = rest.trim_start_matches(['+', ' ']);
    leading_number(rest).map(|(lng, _)| (lat, lng))
}

fn link_point(text: &str) -> Option<(f64, f64)> {
    if let Some(rest) = after(text, "!3d") {
        let (lat, rest) = leading_number(rest)?;
        let (lng, _) = leading_number(after(rest, "!4d")?)?;
        return Some((lat, lng));
    }
    ["?q=", "&q=", "?query=", "&query=", "?ll=", "&ll=", "@"]
        .iter()
        .find_map(|key| after(text, key).and_then(leading_pair))
}

/// Two numbers apart: by a space or a semicolon (each may use a comma as
/// its decimal mark), else by one comma.
fn plain_point(text: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = text
        .split([' ', ';', '\t'])
        .map(|p| p.trim_end_matches(','))
        .filter(|p| !p.is_empty())
        .collect();
    let pair = match parts.as_slice() {
        [lat, lng] => (coordinate(lat), coordinate(lng)),
        [one] => match one.split(',').collect::<Vec<_>>().as_slice() {
            [lat, lng] => (coordinate(lat), coordinate(lng)),
            _ => return None,
        },
        _ => return None,
    };
    (pair.0.is_finite() && pair.1.is_finite()).then_some(pair)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_is_read_as_people_paste_it() {
        use TypedPoint::{Empty, Point, Unreadable};
        let cases = [
            ("", Empty),
            ("  ", Empty),
            ("10.4048, 106.1180", Point(10.4048, 106.118)),
            ("10.4048,106.118", Point(10.4048, 106.118)),
            ("10,4048 106,118", Point(10.4048, 106.118)),
            ("10,4048; 106,118", Point(10.4048, 106.118)),
            ("106.118, 10.4048", Point(10.4048, 106.118)),
            (
                "https://www.google.com/maps/@10.4048,106.118,17z?entry=ttu",
                Point(10.4048, 106.118),
            ),
            (
                "https://www.google.com/maps/place/Cai+Lay/@10.40,106.10,15z/data=!3m1!4b1!4m6!3m5!1s0x0:0x0!8m2!3d10.4048!4d106.118",
                Point(10.4048, 106.118),
            ),
            ("https://maps.google.com/?q=10.4048,106.118", Point(10.4048, 106.118)),
            (
                "https://www.google.com/maps/search/?api=1&query=10.4048%2C106.118",
                Point(10.4048, 106.118),
            ),
            ("https://maps.app.goo.gl/AbCdEf123", Unreadable),
            ("https://maps.google.com/?q=Cai+Lay", Unreadable),
            ("Cai Lậy", Unreadable),
            ("10.4048", Unreadable),
            ("10,4048,106,118", Unreadable),
        ];
        for (text, want) in cases {
            assert_eq!(typed_point(text), want, "{text}");
        }
    }

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
        assert_eq!(vnd_amount("500.000", "price").ok(), Some(Some(500_000)));
        assert_eq!(vnd_amount(" ", "price").ok(), Some(None));
        assert!(vnd_amount("năm trăm", "price").is_err());
        assert_eq!(optional_coordinate(""), None);
        assert!(optional_coordinate("x").is_some_and(f64::is_nan));
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
