//! The rules of a product's boxes, shared by the provider's own forms and the
//! admin's, so the two can never disagree.

use domain::{
    repositories::product_repository_trait::ProductRepositoryTrait, ErrorCode, Result, Validation,
};

use crate::{
    constants::{PRODUCT_DESCRIPTION_MAX, PRODUCT_NAME_MAX, PRODUCT_UNIT_MAX},
    validation::{max_chars, max_chars_opt, non_blank, validate_price},
};

/// A product's boxes once its rules pass: trimmed, blanks as `None`.
pub(crate) struct ProductFields {
    pub name: String,
    pub description: Option<String>,
    pub unit: Option<String>,
    /// The commodity it is for; `None`: every one.
    pub commodity_id: Option<i32>,
}

/// Every rule of a product's boxes, reported together; the category must be
/// a shown supply type, the commodity (when one is chosen) a shown one.
pub(crate) async fn validate_product(
    repo: &(dyn ProductRepositoryTrait + Send + Sync),
    category_id: i32,
    commodity_id: Option<i32>,
    name: String,
    description: Option<String>,
    price: Option<i64>,
    unit: Option<String>,
) -> Result<ProductFields> {
    let name = name.trim().to_owned();
    let description = non_blank(description);
    let unit = non_blank(unit);
    let category_ok = repo.check_category_is_product(category_id).await?;
    let commodity_ok = match commodity_id {
        Some(id) => repo.check_commodity_is_shown(id).await?,
        None => true,
    };
    Validation::new()
        .check(name.is_empty(), "name", ErrorCode::NameRequired)
        .rule("name", max_chars(&name, PRODUCT_NAME_MAX))
        .check(
            !category_ok,
            "category_id",
            ErrorCode::InvalidProductCategory,
        )
        .check(!commodity_ok, "commodity_id", ErrorCode::InvalidCommodity)
        .rule("price", validate_price(price))
        .rule("unit", max_chars_opt(&unit, PRODUCT_UNIT_MAX))
        .rule(
            "description",
            max_chars_opt(&description, PRODUCT_DESCRIPTION_MAX),
        )
        .finish()?;
    Ok(ProductFields {
        name,
        description,
        unit,
        commodity_id,
    })
}
