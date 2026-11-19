//! A business profile's boxes as typed, shared by the provider's own
//! dashboard and the admin's provider pages (one rule set, one form:
//! `organisms/business_fields.html`).

use application::{
    inputs::UpdateProfileInput,
    outputs::{AdminProviderDetailDto, ProviderProfileDto},
};
use serde::{Deserialize, Serialize};

use crate::pages::forms::optional_coordinate;

/// The profile form as typed: every box a string, so a bad coordinate comes
/// back as it was entered.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProfileForm {
    pub name: String,
    pub phone: String,
    pub address: String,
    pub description: String,
    pub lat: String,
    pub lng: String,
}

fn coord(v: Option<f64>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

impl ProfileForm {
    /// The owner's stored profile (dashboard).
    pub fn from_dto(p: &ProviderProfileDto) -> Self {
        Self {
            name: p.name.clone(),
            phone: p.phone.clone(),
            address: p.address.clone().unwrap_or_default(),
            description: p.description.clone().unwrap_or_default(),
            lat: coord(p.lat),
            lng: coord(p.lng),
        }
    }

    /// The same profile as the admin's provider page loads it.
    pub fn from_admin(p: &AdminProviderDetailDto) -> Self {
        Self {
            name: p.name.clone(),
            phone: p.phone.clone(),
            address: p.address.clone().unwrap_or_default(),
            description: p.description.clone().unwrap_or_default(),
            lat: coord(p.lat),
            lng: coord(p.lng),
        }
    }

    pub fn input(&self) -> UpdateProfileInput {
        UpdateProfileInput {
            name: self.name.clone(),
            phone: self.phone.clone(),
            description: Some(self.description.clone()),
            address: Some(self.address.clone()),
            lat: optional_coordinate(&self.lat),
            lng: optional_coordinate(&self.lng),
        }
    }
}
