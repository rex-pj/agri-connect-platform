use std::sync::Arc;

use crate::{
    application::{
        inputs::{CreateProductInput, UpdateProductInput, UpdateProfileInput},
        shared::{validate_coordinates, validate_phone},
    },
    domain::{
        models::product::{NewProduct, UpdateProduct},
        repositories::{
            business_repository_trait::BusinessRepositoryTrait,
            product_repository_trait::ProductRepositoryTrait,
        },
    },
    view_models::{
        responses::{ProviderProductDto, ProviderProfileDto},
        AppError,
    },
};

// ── Use case ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ProviderDashboardUseCase {
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    product_repo:  Arc<dyn ProductRepositoryTrait  + Send + Sync>,
}

impl ProviderDashboardUseCase {
    pub fn new(
        business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
        product_repo:  Arc<dyn ProductRepositoryTrait  + Send + Sync>,
    ) -> Self {
        Self { business_repo, product_repo }
    }

    pub async fn get_profile(&self, user_id: i32) -> Result<ProviderProfileDto, AppError> {
        let profile = self
            .business_repo
            .get_profile_by_user(user_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(
                    "Account is not linked to any business.".into(),
                )
            })?;

        Ok(ProviderProfileDto {
            id:          profile.id,
            name:        profile.name,
            phone:       profile.phone,
            email:       profile.email,
            description: profile.description,
            address:     profile.address,
            is_verified: profile.is_verified,
            lat:         profile.lat,
            lng:         profile.lng,
        })
    }

    pub async fn update_profile(
        &self,
        input: UpdateProfileInput,
    ) -> Result<ProviderProfileDto, AppError> {
        if input.name.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Business name must not be empty.".into(),
            ));
        }
        validate_phone(&input.phone)?;
        if let (Some(lat), Some(lng)) = (input.lat, input.lng) {
            validate_coordinates(lat, lng)?;
        }

        let updated = self
            .business_repo
            .update_profile_text(
                input.user_id,
                input.name.trim(),
                input.phone.trim(),
                input.description.as_deref().map(str::trim),
                input.address.as_deref().map(str::trim),
            )
            .await?;

        if !updated {
            return Err(AppError::NotFound(
                "Business not found for update.".into(),
            ));
        }

        if let (Some(lat), Some(lng)) = (input.lat, input.lng) {
            let wkt = format!("SRID=4326;POINT({lng} {lat})");
            self.business_repo.update_location(input.user_id, &wkt).await?;
        }

        self.get_profile(input.user_id).await
    }

    pub async fn list_products(&self, user_id: i32) -> Result<Vec<ProviderProductDto>, AppError> {
        let products = self.product_repo.list_by_user(user_id).await?;

        Ok(products
            .into_iter()
            .map(|p| ProviderProductDto {
                id: p.id, name: p.name, description: p.description,
                price: p.price, unit: p.unit, image_url: p.image_url,
                category: p.category, category_id: p.category_id,
                is_active: p.is_active, created_on: p.created_on,
            })
            .collect())
    }

    pub async fn create_product(
        &self,
        input: CreateProductInput,
    ) -> Result<ProviderProductDto, AppError> {
        let name = input.name.trim().to_owned();
        if name.is_empty() {
            return Err(AppError::BadRequest(
                "Product name must not be empty.".into(),
            ));
        }
        validate_price(input.price)?;

        let cat_ok = self.product_repo.check_category_is_product(input.category_id).await?;
        if !cat_ok {
            return Err(AppError::BadRequest("Invalid product category.".into()));
        }

        let business_id = self
            .business_repo
            .get_business_id_by_user(input.user_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound("Account is not linked to a business.".into())
            })?;

        let new_id = self
            .product_repo
            .create(NewProduct {
                business_id,
                category_id: input.category_id,
                name,
                description: input.description.as_deref().map(str::trim).map(str::to_owned),
                price:       input.price,
                unit:        input.unit.as_deref().map(str::trim).map(str::to_owned),
                image_url:   input.image_url.as_deref().map(str::trim).map(str::to_owned),
                created_by:  input.user_id,
            })
            .await?;

        let product = self
            .product_repo
            .find_provider_product_by_id(new_id)
            .await?
            .ok_or_else(|| {
                AppError::Internal("Product was created but could not be reloaded.".into())
            })?;

        Ok(map_provider_product(product))
    }

    pub async fn update_product(
        &self,
        input: UpdateProductInput,
    ) -> Result<ProviderProductDto, AppError> {
        let name = input.name.trim().to_owned();
        if name.is_empty() {
            return Err(AppError::BadRequest(
                "Product name must not be empty.".into(),
            ));
        }
        validate_price(input.price)?;

        let cat_ok = self.product_repo.check_category_is_product(input.category_id).await?;
        if !cat_ok {
            return Err(AppError::BadRequest("Invalid product category.".into()));
        }

        let updated = self
            .product_repo
            .update(UpdateProduct {
                id:          input.product_id,
                owner_user:  input.user_id,
                category_id: input.category_id,
                name,
                description: input.description.as_deref().map(str::trim).map(str::to_owned),
                price:       input.price,
                unit:        input.unit.as_deref().map(str::trim).map(str::to_owned),
                image_url:   input.image_url.as_deref().map(str::trim).map(str::to_owned),
            })
            .await?;

        if !updated {
            return Err(AppError::NotFound(
                "Product not found or you do not have permission to edit it.".into(),
            ));
        }

        let product = self
            .product_repo
            .find_provider_product_by_id(input.product_id)
            .await?
            .ok_or_else(|| {
                AppError::Internal("Product was updated but could not be reloaded.".into())
            })?;

        Ok(map_provider_product(product))
    }

    pub async fn delete_product(
        &self,
        product_id: i32,
        user_id: i32,
    ) -> Result<(), AppError> {
        let deleted = self.product_repo.soft_delete(product_id, user_id).await?;

        if !deleted {
            return Err(AppError::NotFound(
                "Product not found or you do not have permission to delete it.".into(),
            ));
        }

        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn validate_price(price: Option<i64>) -> Result<(), AppError> {
    if let Some(p) = price {
        if p <= 0 {
            return Err(AppError::BadRequest("Product price must be greater than 0.".into()));
        }
    }
    Ok(())
}

fn map_provider_product(p: crate::domain::models::product::ProviderProduct) -> ProviderProductDto {
    ProviderProductDto {
        id: p.id, name: p.name, description: p.description,
        price: p.price, unit: p.unit, image_url: p.image_url,
        category: p.category, category_id: p.category_id,
        is_active: p.is_active, created_on: p.created_on,
    }
}
