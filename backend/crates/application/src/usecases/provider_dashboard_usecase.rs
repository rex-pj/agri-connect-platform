use std::sync::Arc;

use domain::{
    models::product::{NewProduct, UpdateProduct},
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
    },
    Error, ErrorCode, Result,
};

use crate::{
    authz::Actor,
    catalogue::{
        PERM_OWN_LISTING_CREATE, PERM_OWN_LISTING_DELETE, PERM_OWN_LISTING_UPDATE,
        PERM_OWN_PROFILE_UPDATE,
    },
    inputs::{CreateProductInput, UpdateProductInput, UpdateProfileInput},
    outputs::{ProviderProductDto, ProviderProfileDto},
    validation::{validate_coordinates, validate_phone, validate_price},
};

// ── Use case ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ProviderDashboardUseCase {
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
}

impl ProviderDashboardUseCase {
    pub fn new(
        business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
        product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    ) -> Self {
        Self {
            business_repo,
            product_repo,
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

        Ok(products
            .into_iter()
            .map(|p| ProviderProductDto {
                id: p.id,
                name: p.name,
                description: p.description,
                price: p.price,
                unit: p.unit,
                category: p.category,
                category_id: p.category_id,
                is_active: p.is_active,
                created_on: p.created_on,
            })
            .collect())
    }

    // ── Write (permission-gated) ───────────────────────────────────────────────

    pub async fn update_profile(
        &self,
        actor: &Actor,
        input: UpdateProfileInput,
    ) -> Result<ProviderProfileDto> {
        actor.require(PERM_OWN_PROFILE_UPDATE)?;

        if input.name.trim().is_empty() {
            return Err(Error::new(ErrorCode::BusinessNameRequired));
        }
        validate_phone(&input.phone)?;
        if let (Some(lat), Some(lng)) = (input.lat, input.lng) {
            validate_coordinates(lat, lng)?;
        }

        let updated = self
            .business_repo
            .update_profile_text(
                actor.user_id,
                input.name.trim(),
                input.phone.trim(),
                input.description.as_deref().map(str::trim),
                input.address.as_deref().map(str::trim),
            )
            .await?;

        if !updated {
            return Err(Error::new(ErrorCode::ProviderNotFound));
        }

        if let (Some(lat), Some(lng)) = (input.lat, input.lng) {
            let wkt = format!("SRID=4326;POINT({lng} {lat})");
            self.business_repo
                .update_location(actor.user_id, &wkt)
                .await?;
        }

        self.get_profile(actor).await
    }

    pub async fn create_product(
        &self,
        actor: &Actor,
        input: CreateProductInput,
    ) -> Result<ProviderProductDto> {
        actor.require(PERM_OWN_LISTING_CREATE)?;

        let name = input.name.trim().to_owned();
        if name.is_empty() {
            return Err(Error::new(ErrorCode::NameRequired));
        }
        validate_price(input.price)?;

        let cat_ok = self
            .product_repo
            .check_category_is_product(input.category_id)
            .await?;
        if !cat_ok {
            return Err(Error::new(ErrorCode::InvalidProductCategory));
        }

        let business_id = self
            .business_repo
            .get_business_id_by_user(actor.user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::AccountNotLinkedToBusiness))?;

        let new_id = self
            .product_repo
            .create(NewProduct {
                business_id,
                category_id: input.category_id,
                name,
                description: input
                    .description
                    .as_deref()
                    .map(str::trim)
                    .map(str::to_owned),
                price: input.price,
                unit: input.unit.as_deref().map(str::trim).map(str::to_owned),
                created_by: actor.user_id,
            })
            .await?;

        self.product_repo
            .find_provider_product_by_id(new_id)
            .await?
            .map(map_provider_product)
            .ok_or_else(|| Error::internal("product created but could not be reloaded"))
    }

    pub async fn update_product(
        &self,
        actor: &Actor,
        input: UpdateProductInput,
    ) -> Result<ProviderProductDto> {
        actor.require(PERM_OWN_LISTING_UPDATE)?;

        let name = input.name.trim().to_owned();
        if name.is_empty() {
            return Err(Error::new(ErrorCode::NameRequired));
        }
        validate_price(input.price)?;

        let cat_ok = self
            .product_repo
            .check_category_is_product(input.category_id)
            .await?;
        if !cat_ok {
            return Err(Error::new(ErrorCode::InvalidProductCategory));
        }

        let updated = self
            .product_repo
            .update(UpdateProduct {
                id: input.product_id,
                owner_user: actor.user_id,
                category_id: input.category_id,
                name,
                description: input
                    .description
                    .as_deref()
                    .map(str::trim)
                    .map(str::to_owned),
                price: input.price,
                unit: input.unit.as_deref().map(str::trim).map(str::to_owned),
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
            .map(map_provider_product)
            .ok_or_else(|| Error::internal("product updated but could not be reloaded"))
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

        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn map_provider_product(p: domain::models::product::ProviderProduct) -> ProviderProductDto {
    ProviderProductDto {
        id: p.id,
        name: p.name,
        description: p.description,
        price: p.price,
        unit: p.unit,
        category: p.category,
        category_id: p.category_id,
        is_active: p.is_active,
        created_on: p.created_on,
    }
}
