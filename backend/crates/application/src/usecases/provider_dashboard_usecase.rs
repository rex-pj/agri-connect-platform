use std::sync::Arc;

use chrono::{Duration, Utc};

use domain::{
    constants::INQUIRY_STATUSES,
    events::{Change, DomainEvent, Entity},
    models::{
        product::{NewProduct, UpdateProduct},
        service::{NewService, UpdateOwnService},
    },
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        call_repository_trait::CallRepositoryTrait,
        inquiry_repository_trait::InquiryRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
        service_repository_trait::ServiceRepositoryTrait,
    },
    Error, ErrorCode, Result,
};

use crate::{
    authz::Actor,
    catalogue::{
        PERM_OWN_INQUIRY_MANAGE, PERM_OWN_LISTING_CREATE, PERM_OWN_LISTING_DELETE,
        PERM_OWN_LISTING_UPDATE, PERM_OWN_PROFILE_UPDATE, PERM_OWN_SERVICE_CREATE,
        PERM_OWN_SERVICE_DELETE, PERM_OWN_SERVICE_UPDATE,
    },
    constants::{CALL_STATS_DAYS, PAGE_SIZE},
    inputs::{CreateProductInput, ServiceInput, UpdateProductInput, UpdateProfileInput},
    outputs::{
        OwnInquiriesResponse, OwnInquiryDto, ProviderProductDto, ProviderProfileDto,
        ProviderServiceDto,
    },
    pagination::{paginate, total_pages},
    ports::{EventBus, ProcessedImage},
    settings::SettingsReader,
    usecases::{
        business_rules::validate_profile, media_usecase::MediaUseCase,
        product_rules::validate_product, service_rules::validate_service,
    },
};

// ── Use case ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ProviderDashboardUseCase {
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    service_repo: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
    call_repo: Arc<dyn CallRepositoryTrait + Send + Sync>,
    inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    /// Stores a new product's photo together with the product.
    media: MediaUseCase,
    /// The listing caps (`listings.*_per_business`).
    settings: SettingsReader,
    events: Arc<dyn EventBus>,
}

/// Everything `ProviderDashboardUseCase` depends on, named.
pub struct ProviderDashboardDeps {
    pub business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    pub product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    pub service_repo: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
    pub call_repo: Arc<dyn CallRepositoryTrait + Send + Sync>,
    pub inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    pub media: MediaUseCase,
    pub settings: SettingsReader,
    pub events: Arc<dyn EventBus>,
}

impl ProviderDashboardUseCase {
    pub fn new(d: ProviderDashboardDeps) -> Self {
        Self {
            business_repo: d.business_repo,
            product_repo: d.product_repo,
            service_repo: d.service_repo,
            call_repo: d.call_repo,
            inquiry_repo: d.inquiry_repo,
            media: d.media,
            settings: d.settings,
            events: d.events,
        }
    }

    // ── Inquiries farmers sent to the actor's business ─────────────────────────

    /// One page of the inbox, newest first. Farmers' phone numbers: a
    /// permission, not just a session.
    pub async fn list_own_inquiries(
        &self,
        actor: &Actor,
        page: i64,
    ) -> Result<OwnInquiriesResponse> {
        actor.require(PERM_OWN_INQUIRY_MANAGE)?;
        let (page, offset) = paginate(page, PAGE_SIZE);
        let ((total, new_count), items) = tokio::try_join!(
            self.inquiry_repo.count_for_owner(actor.user_id),
            self.inquiry_repo
                .list_for_owner(actor.user_id, PAGE_SIZE, offset),
        )?;
        Ok(OwnInquiriesResponse {
            items: items.into_iter().map(OwnInquiryDto::from).collect(),
            total,
            new_count,
            page,
            per_page: PAGE_SIZE,
            total_pages: total_pages(total, PAGE_SIZE),
        })
    }

    /// How many inquiries there are and how many still wait, for the
    /// dashboard's link to the inbox.
    pub async fn count_own_inquiries(&self, actor: &Actor) -> Result<(i64, i64)> {
        actor.require(PERM_OWN_INQUIRY_MANAGE)?;
        self.inquiry_repo.count_for_owner(actor.user_id).await
    }

    /// The business the actor manages, if any: a public page of it offers
    /// its owner the way to edit it. A session is enough: it is their own.
    pub async fn own_business_id(&self, actor: &Actor) -> Result<Option<i32>> {
        self.business_repo
            .get_business_id_by_user(actor.user_id)
            .await
    }

    /// Taps on the actor's call buttons in the last `CALL_STATS_DAYS` days;
    /// `None` when the account manages no business. A session is enough: it
    /// is the provider's own number.
    pub async fn count_own_calls(&self, actor: &Actor) -> Result<Option<i64>> {
        let Some(business_id) = self
            .business_repo
            .get_business_id_by_user(actor.user_id)
            .await?
        else {
            return Ok(None);
        };
        let since = Utc::now() - Duration::days(CALL_STATS_DAYS);
        self.call_repo
            .count_since(Some(business_id), since)
            .await
            .map(Some)
    }

    /// Marks one of the actor's inquiries contacted, closed or spam. Someone
    /// else's looks the same as a missing one.
    pub async fn set_own_inquiry_status(
        &self,
        actor: &Actor,
        inquiry_id: i32,
        status: &str,
    ) -> Result<()> {
        actor.require(PERM_OWN_INQUIRY_MANAGE)?;
        if !INQUIRY_STATUSES.contains(&status) {
            return Err(Error::new(ErrorCode::InvalidInquiryStatus));
        }
        if !self
            .inquiry_repo
            .update_status_for_owner(inquiry_id, actor.user_id, status)
            .await?
        {
            return Err(Error::new(ErrorCode::InquiryNotFound));
        }
        self.announce(
            actor,
            Entity::Inquiry,
            inquiry_id,
            Change::StatusChanged {
                status: status.to_owned(),
            },
        )
        .await;
        Ok(())
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
        let profile = validate_profile(input)?;
        let updated = self
            .business_repo
            .update_profile(actor.user_id, &profile)
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
        let fields = validate_product(
            self.product_repo.as_ref(),
            input.category_id,
            input.commodity_id,
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
        let cap = self.settings.load().await?.products_per_business();
        let created = self
            .product_repo
            .create_capped(
                NewProduct {
                    business_id,
                    category_id: input.category_id,
                    commodity_id: fields.commodity_id,
                    name: fields.name,
                    description: fields.description,
                    price: input.price,
                    unit: fields.unit,
                    image_file_id,
                    created_by: actor.user_id,
                },
                cap,
            )
            .await;
        let new_id = match created {
            Ok(Some(id)) => id,
            refused => {
                if let Some(file) = image_file_id {
                    self.media.discard(file).await;
                }
                return Err(match refused {
                    Err(e) => e,
                    Ok(_) => Error::new(ErrorCode::ProductLimitReached).with_param("max", cap),
                });
            }
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
        let fields = validate_product(
            self.product_repo.as_ref(),
            input.category_id,
            input.commodity_id,
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
                commodity_id: fields.commodity_id,
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
        self.announce(actor, Entity::Product, product_id, Change::Deleted)
            .await;

        Ok(())
    }
}

// ── The actor's own services ──────────────────────────────────────────────────
//
// Every write derives the business from the session (never from the request)
// and matches the service by owner in the same statement, so someone else's
// service looks the same as a missing one. `is_active` stays the admin's: an
// edit never shows a service an admin hid.

impl ProviderDashboardUseCase {
    /// The actor's services, hidden ones too. A session is enough, as for
    /// products.
    pub async fn list_services(&self, actor: &Actor) -> Result<Vec<ProviderServiceDto>> {
        let services = self.service_repo.list_by_owner(actor.user_id).await?;
        Ok(services.into_iter().map(ProviderServiceDto::from).collect())
    }

    pub async fn get_service(&self, actor: &Actor, service_id: i32) -> Result<ProviderServiceDto> {
        self.service_repo
            .find_owned(service_id, actor.user_id)
            .await?
            .map(ProviderServiceDto::from)
            .ok_or_else(|| Error::new(ErrorCode::ServiceNotFound))
    }

    /// Creates one of the actor's services, with its photo when given
    /// (checked and re-encoded already by `MediaUseCase::prepare_image`). The
    /// photo is stored only once the boxes pass and goes in with the row; a
    /// business already listing `SERVICES_PER_BUSINESS_MAX` is refused.
    pub async fn create_service(
        &self,
        actor: &Actor,
        input: ServiceInput,
        image: Option<ProcessedImage>,
    ) -> Result<ProviderServiceDto> {
        actor.require(PERM_OWN_SERVICE_CREATE)?;
        let fields = validate_service(
            self.service_repo.as_ref(),
            input.category_id,
            input.name,
            input.description,
            input.price_note,
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
        let cap = self.settings.load().await?.services_per_business();
        let created = self
            .service_repo
            .create_capped(
                NewService {
                    business_id,
                    category_id: input.category_id,
                    name: fields.name,
                    description: fields.description,
                    price_note: fields.price_note,
                    image_file_id,
                    created_by: actor.user_id,
                },
                cap,
            )
            .await;
        let id = match created {
            Ok(Some(id)) => id,
            refused => {
                if let Some(file) = image_file_id {
                    self.media.discard(file).await;
                }
                return Err(match refused {
                    Err(e) => e,
                    Ok(_) => Error::new(ErrorCode::ServiceLimitReached).with_param("max", cap),
                });
            }
        };

        self.announce(actor, Entity::Service, id, Change::Created)
            .await;
        self.get_service(actor, id).await
    }

    /// Edits one of the actor's services; its photo is replaced separately
    /// (`MediaUseCase::attach_service_image`).
    pub async fn update_service(
        &self,
        actor: &Actor,
        service_id: i32,
        input: ServiceInput,
    ) -> Result<ProviderServiceDto> {
        actor.require(PERM_OWN_SERVICE_UPDATE)?;
        let fields = validate_service(
            self.service_repo.as_ref(),
            input.category_id,
            input.name,
            input.description,
            input.price_note,
        )
        .await?;

        let updated = self
            .service_repo
            .update_owned(UpdateOwnService {
                id: service_id,
                owner_user: actor.user_id,
                category_id: input.category_id,
                name: fields.name,
                description: fields.description,
                price_note: fields.price_note,
            })
            .await?;
        if !updated {
            return Err(Error::new(ErrorCode::ServiceNotFound));
        }

        self.announce(actor, Entity::Service, service_id, Change::Updated)
            .await;
        self.get_service(actor, service_id).await
    }

    pub async fn delete_service(&self, actor: &Actor, service_id: i32) -> Result<()> {
        actor.require(PERM_OWN_SERVICE_DELETE)?;
        if !self
            .service_repo
            .soft_delete_owned(service_id, actor.user_id)
            .await?
        {
            return Err(Error::new(ErrorCode::ServiceNotFound));
        }
        // Its image is removed by the file-cleanup subscriber; past inquiries
        // keep their topic and still point at the (soft-deleted) row.
        self.announce(actor, Entity::Service, service_id, Change::Deleted)
            .await;
        Ok(())
    }

    /// Publishes a change the actor made to one of their own records.
    async fn announce(&self, actor: &Actor, entity: Entity, entity_id: i32, change: Change) {
        self.events
            .publish(DomainEvent {
                actor_id: Some(actor.user_id),
                entity,
                entity_id,
                change,
                reason: None,
            })
            .await;
    }
}
