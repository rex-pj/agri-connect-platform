//! A service's boxes, shared by the admin's service forms and the provider's
//! own (`organisms/service_fields.html`): the values as typed, the length
//! limits for `maxlength`, and the categories to choose from.

use application::{
    constants::{SERVICE_DESCRIPTION_MAX, SERVICE_NAME_MAX, SERVICE_PRICE_NOTE_MAX},
    inputs::ServiceInput,
    outputs::CategoryDto,
};
use domain::{constants::CATEGORY_TYPE_SERVICE, Error};
use serde::{Deserialize, Serialize};

use crate::{app_state::AppState, pages::listing::parse_opt};

/// The use case's length limits, for the boxes' `maxlength`.
#[derive(Serialize)]
pub struct Limits {
    pub name: usize,
    pub description: usize,
    pub price_note: usize,
}

pub const LIMITS: Limits = Limits {
    name: SERVICE_NAME_MAX,
    description: SERVICE_DESCRIPTION_MAX,
    price_note: SERVICE_PRICE_NOTE_MAX,
};

/// The form as typed.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ServiceForm {
    pub category_id: String,
    pub name: String,
    pub description: String,
    pub price_note: String,
}

impl ServiceForm {
    /// The stored values, for an edit form shown fresh.
    pub fn stored(
        category_id: i32,
        name: &str,
        description: Option<&str>,
        price_note: Option<&str>,
    ) -> Self {
        Self {
            category_id: category_id.to_string(),
            name: name.to_owned(),
            description: description.unwrap_or_default().to_owned(),
            price_note: price_note.unwrap_or_default().to_owned(),
        }
    }

    /// An unparsable choice becomes 0, which the use case reports as an
    /// invalid category on that field.
    pub fn category_id(&self) -> i32 {
        parse_opt(&self.category_id).unwrap_or(0)
    }

    pub fn input(&self) -> ServiceInput {
        ServiceInput {
            category_id: self.category_id(),
            name: self.name.clone(),
            description: Some(self.description.clone()),
            price_note: Some(self.price_note.clone()),
        }
    }
}

/// The active service categories, in display order.
pub async fn service_categories(state: &AppState) -> Result<Vec<CategoryDto>, Error> {
    state
        .usecases
        .explore
        .get_categories(Some(CATEGORY_TYPE_SERVICE))
        .await
}
