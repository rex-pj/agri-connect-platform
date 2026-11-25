//! A business profile's boxes as typed, shared by the provider's own
//! dashboard and the admin's provider pages (one rule set, one form:
//! `organisms/business_fields.html`).

use application::{
    admin_units::AreaKey,
    inputs::UpdateProfileInput,
    outputs::{AdminProviderDetailDto, ProviderProfileDto},
};
use serde::{Deserialize, Serialize};

use crate::pages::forms::{typed_point, TypedPoint};

/// The profile form as typed: every box a string, so a bad coordinate comes
/// back as it was entered. The place is the ward chosen (`w28439`) and the
/// point the GPS button, the map or a paste put in `coords` ("10.4048,
/// 106.118" or a Google Maps link, `forms::typed_point`); a ward alone places
/// the business at its centre.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProfileForm {
    pub name: String,
    pub phone: String,
    pub address: String,
    pub description: String,
    pub ward: String,
    pub coords: String,
}

/// A stored point as its box shows it: "10.4048, 106.118".
fn coords(lat: Option<f64>, lng: Option<f64>) -> String {
    lat.zip(lng)
        .map(|(lat, lng)| format!("{lat}, {lng}"))
        .unwrap_or_default()
}

fn ward(code: Option<&str>) -> String {
    code.map(|c| AreaKey::Ward(c.to_owned()).value())
        .unwrap_or_default()
}

impl ProfileForm {
    /// The owner's stored profile (dashboard).
    pub fn from_dto(p: &ProviderProfileDto) -> Self {
        Self {
            name: p.name.clone(),
            phone: p.phone.clone(),
            address: p.address.clone().unwrap_or_default(),
            description: p.description.clone().unwrap_or_default(),
            ward: ward(p.ward_code.as_deref()),
            coords: coords(p.lat, p.lng),
        }
    }

    /// The same profile as the admin's provider page loads it.
    pub fn from_admin(p: &AdminProviderDetailDto) -> Self {
        Self {
            name: p.name.clone(),
            phone: p.phone.clone(),
            address: p.address.clone().unwrap_or_default(),
            description: p.description.clone().unwrap_or_default(),
            ward: ward(p.ward_code.as_deref()),
            coords: coords(p.lat, p.lng),
        }
    }

    pub fn input(&self) -> UpdateProfileInput {
        // Text that is no point is refused by the use case's coordinate
        // rule, with every other message.
        let (lat, lng) = match typed_point(&self.coords) {
            TypedPoint::Empty => (None, None),
            TypedPoint::Point(lat, lng) => (Some(lat), Some(lng)),
            TypedPoint::Unreadable => (Some(f64::NAN), Some(f64::NAN)),
        };
        UpdateProfileInput {
            name: self.name.clone(),
            phone: self.phone.clone(),
            description: Some(self.description.clone()),
            address: Some(self.address.clone()),
            lat,
            lng,
            ward: Some(self.ward.clone()),
        }
    }

    /// The ward code of the chosen ward (`28439`), for the box's name line.
    pub fn ward_code(&self) -> Option<String> {
        match AreaKey::parse(&self.ward) {
            Some(AreaKey::Ward(code)) => Some(code),
            _ => None,
        }
    }

    /// The point typed, when the box holds one.
    pub fn point(&self) -> Option<(f64, f64)> {
        match typed_point(&self.coords) {
            TypedPoint::Point(lat, lng) => Some((lat, lng)),
            _ => None,
        }
    }
}
