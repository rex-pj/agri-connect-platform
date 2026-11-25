//! The rules of a business profile's boxes, shared by the provider's own form
//! and the admin's (a listing set up in the field), so the two can never
//! disagree.

use domain::{models::business::ProfileUpdate, ErrorCode, Result, Validation};

use crate::{
    constants::{BUSINESS_ADDRESS_MAX, BUSINESS_DESCRIPTION_MAX, BUSINESS_NAME_MAX},
    inputs::UpdateProfileInput,
    validation::{
        max_chars, max_chars_opt, non_blank, point_wkt, validate_coordinates, validate_phone,
    },
};

/// Every rule of a profile's boxes, reported together; the profile as stored
/// once they pass (trimmed, blanks as `None`).
pub(crate) fn validate_profile(input: UpdateProfileInput) -> Result<ProfileUpdate> {
    let name = input.name.trim().to_owned();
    let phone = input.phone.trim().to_owned();
    let description = non_blank(input.description);
    let address = non_blank(input.address);
    // Both or neither: one coordinate alone is not a place. Neither means
    // no location, so a wrong pin can be taken off the map.
    let position = match (input.lat, input.lng) {
        (Some(lat), Some(lng)) => Some((lat, lng)),
        (None, None) => None,
        _ => Some((f64::NAN, f64::NAN)),
    };
    Validation::new()
        .check(name.is_empty(), "name", ErrorCode::BusinessNameRequired)
        .rule("name", max_chars(&name, BUSINESS_NAME_MAX))
        .rule("phone", validate_phone(&phone))
        .rule("address", max_chars_opt(&address, BUSINESS_ADDRESS_MAX))
        .rule(
            "description",
            max_chars_opt(&description, BUSINESS_DESCRIPTION_MAX),
        )
        .rule(
            "lat",
            position.map_or(Ok(()), |(lat, lng)| validate_coordinates(lat, lng)),
        )
        .finish()?;
    Ok(ProfileUpdate {
        name,
        phone,
        description,
        address,
        location_wkt: position.map(|(lat, lng)| point_wkt(lat, lng)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(lat: Option<f64>, lng: Option<f64>) -> UpdateProfileInput {
        UpdateProfileInput {
            name: "  Drone Ba Tu  ".into(),
            phone: "0912345678".into(),
            description: Some("   ".into()),
            address: None,
            lat,
            lng,
            ward: None,
        }
    }

    #[test]
    fn trims_and_drops_blanks() -> Result<()> {
        let p = validate_profile(input(None, None))?;
        assert_eq!(p.name, "Drone Ba Tu");
        assert_eq!(p.description, None);
        assert_eq!(p.location_wkt, None);
        Ok(())
    }

    #[test]
    fn one_coordinate_alone_is_refused() {
        let fields = validate_profile(input(Some(10.4), None))
            .err()
            .map(|e| e.fields().iter().map(|f| f.field).collect::<Vec<_>>());
        assert_eq!(fields, Some(vec!["lat"]));
    }

    #[test]
    fn a_pair_becomes_a_point() -> Result<()> {
        let p = validate_profile(input(Some(10.4), Some(106.1)))?;
        assert_eq!(
            p.location_wkt.as_deref(),
            Some("SRID=4326;POINT(106.1 10.4)")
        );
        Ok(())
    }
}
