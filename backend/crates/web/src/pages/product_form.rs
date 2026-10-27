//! A product's boxes, shared by the provider's own product form and the
//! admin's (`organisms/product_fields.html`): the values as typed, parsed
//! into the use case's input, and the categories to choose from.

use application::{
    inputs::{CreateProductInput, UpdateProductInput},
    outputs::CategoryDto,
};
use domain::{constants::CATEGORY_TYPE_PRODUCT, Error};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    pages::{forms::vnd_amount, listing::parse_opt},
};

/// The product form as typed (a photo, where there is one, arrives
/// separately).
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProductForm {
    pub category_id: String,
    pub name: String,
    pub price: String,
    pub unit: String,
    pub description: String,
}

impl ProductForm {
    /// The stored values, for an edit form shown fresh.
    pub fn stored(
        category_id: i32,
        name: &str,
        price: Option<i64>,
        unit: Option<&str>,
        description: Option<&str>,
    ) -> Self {
        Self {
            category_id: category_id.to_string(),
            name: name.to_owned(),
            price: price.map(|v| v.to_string()).unwrap_or_default(),
            unit: unit.unwrap_or_default().to_owned(),
            description: description.unwrap_or_default().to_owned(),
        }
    }

    /// An unparsable category becomes 0, which the use case reports on that
    /// field; a price that is not a number is a field error here.
    pub fn create_input(&self) -> Result<CreateProductInput, Error> {
        Ok(CreateProductInput {
            category_id: parse_opt(&self.category_id).unwrap_or(0),
            name: self.name.clone(),
            description: Some(self.description.clone()),
            price: vnd_amount(&self.price, "price")?,
            unit: Some(self.unit.clone()),
        })
    }

    pub fn update_input(&self, product_id: i32) -> Result<UpdateProductInput, Error> {
        let CreateProductInput {
            category_id,
            name,
            description,
            price,
            unit,
        } = self.create_input()?;
        Ok(UpdateProductInput {
            product_id,
            category_id,
            name,
            description,
            price,
            unit,
        })
    }
}

/// The active product categories, in display order.
pub async fn product_categories(state: &AppState) -> Result<Vec<CategoryDto>, Error> {
    state
        .usecases
        .explore
        .get_categories(Some(CATEGORY_TYPE_PRODUCT))
        .await
}
