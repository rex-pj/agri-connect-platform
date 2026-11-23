//! The rules of a service's boxes, shared by the admin's forms and the
//! provider's own, so the two can never disagree.

use domain::{
    repositories::service_repository_trait::ServiceRepositoryTrait, ErrorCode, Result, Validation,
};

use crate::{
    constants::{SERVICE_DESCRIPTION_MAX, SERVICE_NAME_MAX, SERVICE_PRICE_NOTE_MAX},
    validation::{max_chars, max_chars_opt, non_blank},
};

/// A service's boxes once its rules pass: trimmed, blanks as `None`.
pub(crate) struct ServiceFields {
    pub name: String,
    pub description: Option<String>,
    pub price_note: Option<String>,
}

/// Every rule of a service's boxes, reported together. The category must be
/// an active service category; a blank name takes the category's ("Múc bùn"),
/// so a provider on a phone types as little as possible.
pub(crate) async fn validate_service(
    repo: &(dyn ServiceRepositoryTrait + Send + Sync),
    category_id: i32,
    name: String,
    description: Option<String>,
    price_note: Option<String>,
) -> Result<ServiceFields> {
    let category = repo.active_service_category_name(category_id).await?;
    let typed = name.trim();
    let name = match (typed.is_empty(), &category) {
        (true, Some(category)) => category.clone(),
        _ => typed.to_owned(),
    };
    let description = non_blank(description);
    let price_note = non_blank(price_note);
    Validation::new()
        .check(name.is_empty(), "name", ErrorCode::NameRequired)
        .rule("name", max_chars(&name, SERVICE_NAME_MAX))
        .check(
            category.is_none(),
            "category_id",
            ErrorCode::InvalidServiceCategory,
        )
        .rule(
            "description",
            max_chars_opt(&description, SERVICE_DESCRIPTION_MAX),
        )
        .rule(
            "price_note",
            max_chars_opt(&price_note, SERVICE_PRICE_NOTE_MAX),
        )
        .finish()?;
    Ok(ServiceFields {
        name,
        description,
        price_note,
    })
}
