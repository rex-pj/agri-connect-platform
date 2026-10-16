use std::sync::Arc;

use domain::{
    events::{Change, DomainEvent, Entity},
    models::{
        business::ProfileUpdate,
        product::{NewProduct, UpdateProduct},
    },
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::{
        PERM_OWN_LISTING_CREATE, PERM_OWN_LISTING_DELETE, PERM_OWN_LISTING_UPDATE,
        PERM_OWN_PROFILE_UPDATE,
    },
    constants::{
        BUSINESS_ADDRESS_MAX, BUSINESS_DESCRIPTION_MAX, BUSINESS_NAME_MAX, PRODUCT_DESCRIPTION_MAX,
        PRODUCT_NAME_MAX, PRODUCT_UNIT_MAX,
    },
    inputs::{CreateProductInput, UpdateProductInput, UpdateProfileInput},
    outputs::{ProviderProductDto, ProviderProfileDto},
    ports::{EventBus, ProcessedImage},
    usecases::media_usecase::MediaUseCase,
    validation::{
        max_chars, max_chars_opt, non_blank, point_wkt, validate_coordinates, validate_phone,
        validate_price,
    },
};

// ── Use case ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ProviderDashboardUseCase {
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    /// Stores a new product's photo together with the product.
    media: MediaUseCase,
    events: Arc<dyn EventBus>,
}

impl ProviderDashboardUseCase {
    pub fn new(
        business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
        product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
        media: MediaUseCase,
        events: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            business_repo,
            product_repo,
            media,
            events,
        }
    }

    // Reading your own business needs a session but no permission: a provider
    // whose write permissions were revoked can still see what is listed.

    pub async fn get_profile(&self, actor: &Actor) -> Result<ProviderProfileDto> {
        let profile = self
            .business_repo
            .get_profile_by_user(actor.user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::AccountNotLinkedToBusiness))?;

        Ok(ProviderProfileDto {
            id: profile.id,
            name: profile.name,
            phone: profile.phone,
            email: profile.email,
            description: profile.description,
            address: profile.address,
            is_verified: profile.is_verified,
            lat: profile.lat,
            lng: profile.lng,
        })
    }

    pub async fn list_products(&self, actor: &Actor) -> Result<Vec<ProviderProductDto>> {
        let products = self.product_repo.list_by_user(actor.user_id).await?;

        Ok(products.into_iter().map(ProviderProductDto::from).collect())
    }

    /// One of the actor's own products. Someone else's looks the same as a
    /// missing one, so ids cannot be probed for ownership.
    pub async fn get_product(&self, actor: &Actor, product_id: i32) -> Result<ProviderProductDto> {
        self.product_repo
            .list_by_user(actor.user_id)
            .await?
            .into_iter()
            .find(|p| p.id == product_id)
            .map(ProviderProductDto::from)
            .ok_or_else(|| Error::new(ErrorCode::ProductNotFound))
    }

    // ── Write (permission-gated) ───────────────────────────────────────────────

    pub async fn update_profile(
        &self,
        actor: &Actor,
        input: UpdateProfileInput,
    ) -> Result<ProviderProfileDto> {
        actor.require(PERM_OWN_PROFILE_UPDATE)?;

        let name = input.name.trim().to_owned();
        let phone = input.phone.trim().to_owned();
        let description = non_blank(input.description);
        let address = non_blank(input.address);
        // Both or neither: one coordinate alone is not a place. Neither means
        // no location, so a provider can take a wrong pin off the map.
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

        let updated = self
            .business_repo
            .update_profile(
                actor.user_id,
                &ProfileUpdate {
                    name,
                    phone,
                    description,
                    address,
                    location_wkt: position.map(|(lat, lng)| point_wkt(lat, lng)),
                },
            )
            .await?;

        if !updated {
            return Err(Error::new(ErrorCode::ProviderNotFound));
        }

        self.get_profile(actor).await
    }

    /// Creates one of the actor's products, with its photo when given (checked
    /// and re-encoded already by `MediaUseCase::prepare_image`). The photo is
    /// stored only once the boxes pass, and goes in with the row: there is
    /// never a product left without the photo that was sent with it, nor a
    /// stored photo without its product.
    pub async fn create_product(
        &self,
        actor: &Actor,
        input: CreateProductInput,
        image: Option<ProcessedImage>,
    ) -> Result<ProviderProductDto> {
        actor.require(PERM_OWN_LISTING_CREATE)?;
        let fields = self
            .validate_product(
                input.category_id,
                input.name,
                input.description,
                input.price,
                input.unit,
            )
            .await?;

        let business_id = self
            .business_repo
            .get_business_id_by_user(actor.user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::AccountNotLinkedToBusiness))?;

        let image_file_id = match image {
            Some(image) => Some(self.media.store(actor, image).await?),
            None => None,
        };
        let created = self
            .product_repo
            .create(NewProduct {
                business_id,
                category_id: input.category_id,
                name: fields.name,
                description: fields.description,
                price: input.price,
                unit: fields.unit,
                image_file_id,
                created_by: actor.user_id,
            })
            .await;
        let new_id = match (created, image_file_id) {
            (Ok(id), _) => id,
            (Err(e), Some(file)) => {
                self.media.discard(file).await;
                return Err(e);
            }
            (Err(e), None) => return Err(e),
        };

        self.product_repo
            .find_provider_product_by_id(new_id)
            .await?
            .map(ProviderProductDto::from)
            .ok_or_else(|| Error::internal("product created but could not be reloaded"))
    }

    pub async fn update_product(
        &self,
        actor: &Actor,
        input: UpdateProductInput,
    ) -> Result<ProviderProductDto> {
        actor.require(PERM_OWN_LISTING_UPDATE)?;
        let fields = self
            .validate_product(
                input.category_id,
                input.name,
                input.description,
                input.price,
                input.unit,
            )
            .await?;

        let updated = self
            .product_repo
            .update(UpdateProduct {
                id: input.product_id,
                owner_user: actor.user_id,
                category_id: input.category_id,
                name: fields.name,
                description: fields.description,
                price: input.price,
                unit: fields.unit,
            })
            .await?;

        if !updated {
            // Someone else's product looks the same as a missing one, so ids
            // cannot be probed for ownership.
            return Err(Error::new(ErrorCode::ProductNotFound));
        }

        self.product_repo
            .find_provider_product_by_id(input.product_id)
            .await?
            .map(ProviderProductDto::from)
            .ok_or_else(|| Error::internal("product updated but could not be reloaded"))
    }

    /// Every rule of a product's boxes, reported together; the category must
    /// be an active product category.
    async fn validate_product(
        &self,
        category_id: i32,
        name: String,
        description: Option<String>,
        price: Option<i64>,
        unit: Option<String>,
    ) -> Result<ProductFields> {
        let name = name.trim().to_owned();
        let description = non_blank(description);
        let unit = non_blank(unit);
        let category_ok = self
            .product_repo
            .check_category_is_product(category_id)
            .await?;
        Validation::new()
            .check(name.is_empty(), "name", ErrorCode::NameRequired)
            .rule("name", max_chars(&name, PRODUCT_NAME_MAX))
            .check(
                !category_ok,
                "category_id",
                ErrorCode::InvalidProductCategory,
            )
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
        })
    }

    pub async fn delete_product(&self, actor: &Actor, product_id: i32) -> Result<()> {
        actor.require(PERM_OWN_LISTING_DELETE)?;

        let deleted = self
            .product_repo
            .soft_delete(product_id, actor.user_id)
            .await?;

        if !deleted {
            // Someone else's product looks the same as a missing one, so ids
            // cannot be probed for ownership.
            return Err(Error::new(ErrorCode::ProductNotFound));
        }

        // Its image is removed by the file-cleanup subscriber.
        self.events
            .publish(DomainEvent {
                actor_id: actor.user_id,
                entity: Entity::Product,
                entity_id: product_id,
                change: Change::Deleted,
                reason: None,
            })
            .await;

        Ok(())
    }
}

/// A product's boxes once its rules pass: trimmed, blanks as `None`.
struct ProductFields {
    name: String,
    description: Option<String>,
    unit: Option<String>,
}
