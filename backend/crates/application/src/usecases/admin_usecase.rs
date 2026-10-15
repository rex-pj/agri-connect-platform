use std::sync::Arc;

use domain::{
    constants::{CATEGORY_TYPES, INQUIRY_STATUSES, ROLE_PROVIDER},
    events::{Change, DomainEvent, Entity},
    models::{
        category::{CategoryAdmin, NewCategory, UpdateCategory},
        location::Location,
        role::UserRole,
        service::{AdminService, NewService, UpdateService},
        user::NewUser,
    },
    repositories::{
        business_repository_trait::BusinessRepositoryTrait,
        category_repository_trait::CategoryRepositoryTrait,
        inquiry_repository_trait::InquiryRepositoryTrait,
        location_repository_trait::LocationRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
        review_repository_trait::ReviewRepositoryTrait, role_repository_trait::RoleRepositoryTrait,
        service_repository_trait::ServiceRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    authz::Actor,
    catalogue::{
        PERM_CATEGORY_MANAGE, PERM_INQUIRY_MANAGE, PERM_INQUIRY_READ, PERM_LISTING_MODERATE,
        PERM_LOCATION_MANAGE, PERM_PROVIDER_ACCOUNT_ASSIGN, PERM_PROVIDER_READ,
        PERM_PROVIDER_VERIFY, PERM_REVIEW_MODERATE, PERM_ROLE_READ, PERM_SERVICE_MANAGE,
        PERM_USER_READ, PERM_USER_ROLE_UPDATE, PERM_USER_STATUS_UPDATE,
    },
    constants::{
        ADMIN_PAGE_SIZE, CATEGORY_COLOR_MAX, CATEGORY_DESCRIPTION_MAX, CATEGORY_ICON_MAX,
        CATEGORY_NAME_MAX, CATEGORY_SLUG_MAX, LOCATION_LABEL_MAX,
    },
    inputs::CategoryInput,
    outputs::{
        AdminCategoryDto, AdminInquiryDto, AdminLocationDto, AdminPagedResponse, AdminProductDto,
        AdminProductItemDto, AdminProviderDetailDto, AdminProviderDto, AdminReviewDto,
        AdminServiceDto, AdminServiceItemDto, AdminUserDetailDto, AdminUserDto, AssignUserDto,
        RoleDto, VerifyToggleDto,
    },
    pagination::{paginate, total_pages},
    ports::{EventBus, PasswordHasher},
    validation::{
        max_chars, non_blank, validate_coordinates, validate_password, validate_slug,
        validate_username,
    },
};

// ── Use case ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AdminUseCase {
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    review_repo: Arc<dyn ReviewRepositoryTrait + Send + Sync>,
    product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    role_repo: Arc<dyn RoleRepositoryTrait + Send + Sync>,
    service_repo: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
    events: Arc<dyn EventBus>,
    /// Password hashing injected via port — infrastructure stays out of use cases.
    password_hasher: Arc<dyn PasswordHasher>,
}

/// Everything `AdminUseCase` depends on, named so wiring cannot swap two repos
/// of compatible types by position.
pub struct AdminDeps {
    pub business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    pub user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    pub category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    pub inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    pub location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    pub review_repo: Arc<dyn ReviewRepositoryTrait + Send + Sync>,
    pub product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    pub role_repo: Arc<dyn RoleRepositoryTrait + Send + Sync>,
    pub service_repo: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
    pub events: Arc<dyn EventBus>,
    pub password_hasher: Arc<dyn PasswordHasher>,
}

impl AdminUseCase {
    pub fn new(d: AdminDeps) -> Self {
        Self {
            business_repo: d.business_repo,
            user_repo: d.user_repo,
            category_repo: d.category_repo,
            inquiry_repo: d.inquiry_repo,
            location_repo: d.location_repo,
            review_repo: d.review_repo,
            product_repo: d.product_repo,
            role_repo: d.role_repo,
            service_repo: d.service_repo,
            events: d.events,
            password_hasher: d.password_hasher,
        }
    }

    /// Announces a change that is already stored. Subscribers (the audit log)
    /// cannot fail it.
    async fn publish(
        &self,
        actor: &Actor,
        entity: Entity,
        entity_id: i32,
        change: Change,
        reason: Option<String>,
    ) {
        self.events
            .publish(DomainEvent {
                actor_id: actor.user_id,
                entity,
                entity_id,
                change,
                reason,
            })
            .await;
    }

    // ── Providers ─────────────────────────────────────────────────────────────

    pub async fn get_provider_detail(
        &self,
        actor: &Actor,
        id: i32,
    ) -> Result<AdminProviderDetailDto> {
        actor.require(PERM_PROVIDER_READ)?;
        let b = self
            .business_repo
            .find_admin_detail(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ProviderNotFound))?;

        Ok(AdminProviderDetailDto {
            id: b.id,
            name: b.name,
            phone: b.phone,
            email: b.email,
            description: b.description,
            address: b.address,
            is_verified: b.is_verified,
            is_active: b.is_active,
            created_on: b.created_on,
            linked_username: b.linked_username,
            services: b
                .services
                .into_iter()
                .map(|s| AdminServiceItemDto {
                    id: s.id,
                    name: s.name,
                    category: s.category,
                    price_note: s.price_note,
                    is_active: s.is_active,
                })
                .collect(),
            products: b
                .products
                .into_iter()
                .map(|p| AdminProductItemDto {
                    id: p.id,
                    name: p.name,
                    price: p.price,
                    unit: p.unit,
                    category: p.category,
                    is_active: p.is_active,
                })
                .collect(),
        })
    }

    pub async fn list_providers(
        &self,
        actor: &Actor,
        page: i64,
    ) -> Result<AdminPagedResponse<AdminProviderDto>> {
        actor.require(PERM_PROVIDER_READ)?;
        let (page, offset) = paginate(page, ADMIN_PAGE_SIZE);
        let total = self.business_repo.count_all().await?;
        let items = self
            .business_repo
            .list_for_admin(ADMIN_PAGE_SIZE, offset)
            .await?;

        Ok(AdminPagedResponse {
            items: items
                .into_iter()
                .map(|b| AdminProviderDto {
                    id: b.id,
                    name: b.name,
                    phone: b.phone,
                    address: b.address,
                    is_verified: b.is_verified,
                    is_active: b.is_active,
                    service_count: b.service_count,
                    product_count: b.product_count,
                    created_on: b.created_on,
                    updated_on: b.updated_on,
                    linked_username: b.linked_username,
                    created_by_name: b.created_by_name,
                    updated_by_name: b.updated_by_name,
                })
                .collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: total_pages(total, ADMIN_PAGE_SIZE),
        })
    }

    pub async fn toggle_verify(
        &self,
        actor: &Actor,
        id: i32,
        reason: Option<String>,
    ) -> Result<VerifyToggleDto> {
        actor.require(PERM_PROVIDER_VERIFY)?;
        let result = self
            .business_repo
            .toggle_verify(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ProviderNotFound))?;

        let (id, name, is_verified) = result;
        self.publish(
            actor,
            Entity::Provider,
            id,
            Change::Visibility {
                visible: is_verified,
            },
            reason,
        )
        .await;
        Ok(VerifyToggleDto {
            id,
            name,
            is_verified,
        })
    }

    pub async fn assign_provider_user(
        &self,
        actor: &Actor,
        business_id: i32,
        username: String,
        email: String,
        password: String,
    ) -> Result<AssignUserDto> {
        actor.require(PERM_PROVIDER_ACCOUNT_ASSIGN)?;
        let username = username.trim().to_lowercase();
        Validation::new()
            .rule("username", validate_username(&username))
            .rule("password", validate_password(&password))
            .finish()?;

        let biz_exists = self
            .business_repo
            .check_exists_for_admin(business_id)
            .await?;
        if !biz_exists {
            return Err(Error::new(ErrorCode::ProviderNotFound));
        }

        // An existing provider account is linked as-is; its password is left
        // alone. Any other role is refused so an admin cannot be demoted here.
        let user_id = match self.user_repo.find_by_username(&username).await? {
            Some(u) if u.role != ROLE_PROVIDER => {
                return Err(Error::new(ErrorCode::UsernameTakenByOtherRole));
            }
            Some(u) => u.id,
            None => {
                let password_hash = self.password_hasher.hash(&password).await?;
                self.user_repo
                    .create(NewUser {
                        username: username.clone(),
                        email: email.trim().to_lowercase(),
                        password_hash,
                        role: ROLE_PROVIDER.into(),
                        full_name: None,
                    })
                    .await?
            }
        };

        self.business_repo.assign_user(business_id, user_id).await?;

        Ok(AssignUserDto {
            business_id,
            username,
        })
    }

    // ── Users ─────────────────────────────────────────────────────────────────

    pub async fn get_user_detail(&self, actor: &Actor, id: i32) -> Result<AdminUserDetailDto> {
        actor.require(PERM_USER_READ)?;
        self.load_user_detail(id).await
    }

    async fn load_user_detail(&self, id: i32) -> Result<AdminUserDetailDto> {
        let u = self
            .user_repo
            .find_by_id_for_admin(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::UserNotFound))?;

        Ok(AdminUserDetailDto {
            id: u.id,
            username: u.username,
            email: u.email,
            full_name: u.full_name,
            role_code: u.role_code,
            role_name: u.role_name,
            is_active: u.is_active,
            created_on: u.created_on,
        })
    }

    pub async fn list_users(
        &self,
        actor: &Actor,
        page: i64,
    ) -> Result<AdminPagedResponse<AdminUserDto>> {
        actor.require(PERM_USER_READ)?;
        let (page, offset) = paginate(page, ADMIN_PAGE_SIZE);
        let total = self.user_repo.count_all().await?;
        let items = self
            .user_repo
            .list_for_admin(ADMIN_PAGE_SIZE, offset)
            .await?;

        Ok(AdminPagedResponse {
            items: items
                .into_iter()
                .map(|u| AdminUserDto {
                    id: u.id,
                    username: u.username,
                    email: u.email,
                    full_name: u.full_name,
                    role_code: u.role_code,
                    role_name: u.role_name,
                    is_active: u.is_active,
                    created_on: u.created_on,
                    updated_on: u.updated_on,
                    created_by_name: u.created_by_name,
                    updated_by_name: u.updated_by_name,
                })
                .collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: total_pages(total, ADMIN_PAGE_SIZE),
        })
    }

    pub async fn update_user_role(
        &self,
        actor: &Actor,
        user_id: i32,
        role_code: String,
    ) -> Result<AdminUserDetailDto> {
        actor.require(PERM_USER_ROLE_UPDATE)?;
        refuse_self(actor, user_id)?;
        // Both checks matter: the roles table says the role is active, the enum
        // says the code knows how to authorize it. Arbitrary strings must never
        // reach the role column.
        let code = role_code.trim();
        if self.role_repo.find_active_by_code(code).await?.is_none() {
            return Err(Error::new(ErrorCode::InvalidRole));
        }
        let role = UserRole::from_code(code).ok_or_else(|| Error::new(ErrorCode::InvalidRole))?;
        self.user_repo.update_role(user_id, &role).await?;
        self.publish(
            actor,
            Entity::User,
            user_id,
            Change::RoleChanged {
                role: code.to_owned(),
            },
            None,
        )
        .await;
        self.load_user_detail(user_id).await
    }

    pub async fn toggle_user_active(
        &self,
        actor: &Actor,
        user_id: i32,
        is_active: bool,
        reason: Option<String>,
    ) -> Result<AdminUserDetailDto> {
        actor.require(PERM_USER_STATUS_UPDATE)?;
        refuse_self(actor, user_id)?;
        self.user_repo.toggle_active(user_id, is_active).await?;
        self.publish(
            actor,
            Entity::User,
            user_id,
            Change::Visibility { visible: is_active },
            reason,
        )
        .await;
        self.load_user_detail(user_id).await
    }

    // ── Categories ────────────────────────────────────────────────────────────

    pub async fn get_category_detail(&self, actor: &Actor, id: i32) -> Result<AdminCategoryDto> {
        actor.require(PERM_CATEGORY_MANAGE)?;
        self.load_category_detail(id).await
    }

    async fn load_category_detail(&self, id: i32) -> Result<AdminCategoryDto> {
        self.category_repo
            .find_by_id_admin(id)
            .await?
            .map(to_category_dto)
            .ok_or_else(|| Error::new(ErrorCode::CategoryNotFound))
    }

    pub async fn list_categories(&self, actor: &Actor) -> Result<Vec<AdminCategoryDto>> {
        actor.require(PERM_CATEGORY_MANAGE)?;
        let cats = self.category_repo.list_admin().await?;
        Ok(cats.into_iter().map(to_category_dto).collect())
    }

    pub async fn create_category(
        &self,
        actor: &Actor,
        category_type: String,
        input: CategoryInput,
    ) -> Result<AdminCategoryDto> {
        actor.require(PERM_CATEGORY_MANAGE)?;
        let input = validate_category(input, Some(&category_type))?;
        let id = self
            .category_repo
            .create(NewCategory {
                name: input.name,
                slug: input.slug,
                category_type,
                icon: input.icon,
                color: input.color,
                description: input.description,
                display_order: input.display_order,
                created_by: actor.user_id,
            })
            .await?;
        self.load_category_detail(id).await
    }

    pub async fn update_category(
        &self,
        actor: &Actor,
        id: i32,
        input: CategoryInput,
    ) -> Result<AdminCategoryDto> {
        actor.require(PERM_CATEGORY_MANAGE)?;
        let input = validate_category(input, None)?;
        self.category_repo
            .update(
                id,
                UpdateCategory {
                    name: input.name,
                    slug: input.slug,
                    icon: input.icon,
                    color: input.color,
                    description: input.description,
                    display_order: input.display_order,
                    updated_by: actor.user_id,
                },
            )
            .await?;
        self.load_category_detail(id).await
    }

    pub async fn toggle_category_active(
        &self,
        actor: &Actor,
        id: i32,
        is_active: bool,
        reason: Option<String>,
    ) -> Result<AdminCategoryDto> {
        actor.require(PERM_CATEGORY_MANAGE)?;
        self.category_repo
            .toggle_active(id, is_active, actor.user_id)
            .await?;
        self.publish(
            actor,
            Entity::Category,
            id,
            Change::Visibility { visible: is_active },
            reason,
        )
        .await;
        self.load_category_detail(id).await
    }

    // ── Inquiries ─────────────────────────────────────────────────────────────

    pub async fn list_inquiries(
        &self,
        actor: &Actor,
        page: i64,
    ) -> Result<AdminPagedResponse<AdminInquiryDto>> {
        actor.require(PERM_INQUIRY_READ)?;
        let (page, offset) = paginate(page, ADMIN_PAGE_SIZE);
        let total = self.inquiry_repo.count_admin().await?;
        let items = self
            .inquiry_repo
            .list_admin(ADMIN_PAGE_SIZE, offset)
            .await?;

        Ok(AdminPagedResponse {
            items: items
                .into_iter()
                .map(|i| AdminInquiryDto {
                    id: i.id,
                    business_id: i.business_id,
                    business_name: i.business_name,
                    requester_name: i.requester_name,
                    requester_phone: i.requester_phone,
                    topic: i.topic,
                    status: i.status,
                    created_on: i.created_on,
                    updated_on: i.updated_on,
                    updated_by_name: i.updated_by_name,
                })
                .collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: total_pages(total, ADMIN_PAGE_SIZE),
        })
    }

    pub async fn update_inquiry_status(
        &self,
        actor: &Actor,
        id: i32,
        status: String,
    ) -> Result<AdminInquiryDto> {
        actor.require(PERM_INQUIRY_MANAGE)?;
        if !INQUIRY_STATUSES.contains(&status.as_str()) {
            return Err(Error::new(ErrorCode::InvalidInquiryStatus));
        }
        self.inquiry_repo
            .update_status(id, &status, actor.user_id)
            .await?;
        let inquiry = self
            .inquiry_repo
            .find_by_id_admin(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::InquiryNotFound))?;
        Ok(AdminInquiryDto {
            id: inquiry.id,
            business_id: inquiry.business_id,
            business_name: inquiry.business_name,
            requester_name: inquiry.requester_name,
            requester_phone: inquiry.requester_phone,
            topic: inquiry.topic,
            status: inquiry.status,
            created_on: inquiry.created_on,
            updated_on: inquiry.updated_on,
            updated_by_name: inquiry.updated_by_name,
        })
    }

    // ── Reviews ───────────────────────────────────────────────────────────────

    pub async fn list_reviews(
        &self,
        actor: &Actor,
        page: i64,
    ) -> Result<AdminPagedResponse<AdminReviewDto>> {
        actor.require(PERM_REVIEW_MODERATE)?;
        let (page, offset) = paginate(page, ADMIN_PAGE_SIZE);
        let total = self.review_repo.count_admin().await?;
        let items = self.review_repo.list_admin(ADMIN_PAGE_SIZE, offset).await?;

        Ok(AdminPagedResponse {
            items: items
                .into_iter()
                .map(|r| AdminReviewDto {
                    id: r.id,
                    business_id: r.business_id,
                    business_name: r.business_name,
                    reviewer_name: r.reviewer_name,
                    rating: r.rating,
                    comment: r.comment,
                    is_visible: r.is_visible,
                    created_on: r.created_on,
                    updated_on: r.updated_on,
                    updated_by_name: r.updated_by_name,
                })
                .collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: total_pages(total, ADMIN_PAGE_SIZE),
        })
    }

    pub async fn toggle_review_visible(
        &self,
        actor: &Actor,
        id: i32,
        is_visible: bool,
        reason: Option<String>,
    ) -> Result<AdminReviewDto> {
        actor.require(PERM_REVIEW_MODERATE)?;
        self.review_repo
            .toggle_visible(id, is_visible, actor.user_id)
            .await?;
        self.publish(
            actor,
            Entity::Review,
            id,
            Change::Visibility {
                visible: is_visible,
            },
            reason,
        )
        .await;
        let review = self
            .review_repo
            .find_by_id_admin(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ReviewNotFound))?;
        Ok(AdminReviewDto {
            id: review.id,
            business_id: review.business_id,
            business_name: review.business_name,
            reviewer_name: review.reviewer_name,
            rating: review.rating,
            comment: review.comment,
            is_visible: review.is_visible,
            created_on: review.created_on,
            updated_on: review.updated_on,
            updated_by_name: review.updated_by_name,
        })
    }

    // ── Locations ─────────────────────────────────────────────────────────────

    pub async fn list_locations_admin(&self, actor: &Actor) -> Result<Vec<AdminLocationDto>> {
        actor.require(PERM_LOCATION_MANAGE)?;
        self.location_repo
            .list_all()
            .await
            .map(|locs| locs.into_iter().map(to_location_dto).collect())
    }

    pub async fn get_location_admin(&self, actor: &Actor, id: i32) -> Result<AdminLocationDto> {
        actor.require(PERM_LOCATION_MANAGE)?;
        self.load_location_admin(id).await
    }

    async fn load_location_admin(&self, id: i32) -> Result<AdminLocationDto> {
        self.location_repo
            .find_by_id(id)
            .await?
            .map(to_location_dto)
            .ok_or_else(|| Error::new(ErrorCode::LocationNotFound))
    }

    pub async fn create_location(
        &self,
        actor: &Actor,
        label: String,
        lat: f64,
        lng: f64,
        display_order: i32,
    ) -> Result<AdminLocationDto> {
        actor.require(PERM_LOCATION_MANAGE)?;
        let label = validate_location(&label, lat, lng)?;
        let id = self
            .location_repo
            .create(label, lat, lng, display_order)
            .await?;
        self.load_location_admin(id).await
    }

    pub async fn update_location(
        &self,
        actor: &Actor,
        id: i32,
        label: String,
        lat: f64,
        lng: f64,
        display_order: i32,
    ) -> Result<AdminLocationDto> {
        actor.require(PERM_LOCATION_MANAGE)?;
        let label = validate_location(&label, lat, lng)?;
        self.location_repo
            .update(id, label, lat, lng, display_order)
            .await?;
        self.load_location_admin(id).await
    }

    pub async fn toggle_location_active(
        &self,
        actor: &Actor,
        id: i32,
        is_active: bool,
        reason: Option<String>,
    ) -> Result<AdminLocationDto> {
        actor.require(PERM_LOCATION_MANAGE)?;
        self.location_repo.toggle_active(id, is_active).await?;
        self.publish(
            actor,
            Entity::Location,
            id,
            Change::Visibility { visible: is_active },
            reason,
        )
        .await;
        self.load_location_admin(id).await
    }

    pub async fn delete_location(
        &self,
        actor: &Actor,
        id: i32,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_LOCATION_MANAGE)?;
        self.location_repo.delete(id).await?;
        self.publish(actor, Entity::Location, id, Change::Deleted, reason)
            .await;
        Ok(())
    }

    // ── Admin Products ────────────────────────────────────────────────────────

    pub async fn list_products_admin(
        &self,
        actor: &Actor,
        page: i64,
        provider_id: Option<i32>,
    ) -> Result<AdminPagedResponse<AdminProductDto>> {
        actor.require(PERM_LISTING_MODERATE)?;
        let (page, offset) = paginate(page, ADMIN_PAGE_SIZE);
        let total = self.product_repo.count_admin(provider_id).await?;
        let items = self
            .product_repo
            .list_admin(provider_id, ADMIN_PAGE_SIZE, offset)
            .await?;
        Ok(AdminPagedResponse {
            items: items.into_iter().map(AdminProductDto::from).collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: total_pages(total, ADMIN_PAGE_SIZE),
        })
    }

    pub async fn get_product_admin(&self, actor: &Actor, id: i32) -> Result<AdminProductDto> {
        actor.require(PERM_LISTING_MODERATE)?;
        self.load_product_admin(id).await
    }

    async fn load_product_admin(&self, id: i32) -> Result<AdminProductDto> {
        let p = self
            .product_repo
            .find_by_id_admin(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ProductNotFound))?;
        Ok(AdminProductDto::from(p))
    }

    pub async fn toggle_product_active(
        &self,
        actor: &Actor,
        id: i32,
        is_active: bool,
        reason: Option<String>,
    ) -> Result<AdminProductDto> {
        actor.require(PERM_LISTING_MODERATE)?;
        let updated = self.product_repo.admin_toggle_active(id, is_active).await?;
        if !updated {
            return Err(Error::new(ErrorCode::ProductNotFound));
        }
        self.publish(
            actor,
            Entity::Product,
            id,
            Change::Visibility { visible: is_active },
            reason,
        )
        .await;
        self.load_product_admin(id).await
    }

    pub async fn delete_product_admin(
        &self,
        actor: &Actor,
        id: i32,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_LISTING_MODERATE)?;
        let deleted = self.product_repo.admin_delete(id).await?;
        if !deleted {
            return Err(Error::new(ErrorCode::ProductNotFound));
        }
        self.publish(actor, Entity::Product, id, Change::Deleted, reason)
            .await;
        Ok(())
    }

    // ── Admin Services ────────────────────────────────────────────────────────

    pub async fn list_services_admin(
        &self,
        actor: &Actor,
        page: i64,
        provider_id: Option<i32>,
    ) -> Result<AdminPagedResponse<AdminServiceDto>> {
        actor.require(PERM_SERVICE_MANAGE)?;
        let (page, offset) = paginate(page, ADMIN_PAGE_SIZE);
        let total = self.service_repo.count_admin(provider_id).await?;
        let items = self
            .service_repo
            .list_admin(provider_id, ADMIN_PAGE_SIZE, offset)
            .await?;
        Ok(AdminPagedResponse {
            items: items.into_iter().map(to_service_dto).collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: total_pages(total, ADMIN_PAGE_SIZE),
        })
    }

    pub async fn get_service_admin(&self, actor: &Actor, id: i32) -> Result<AdminServiceDto> {
        actor.require(PERM_SERVICE_MANAGE)?;
        self.load_service_admin(id).await
    }

    async fn load_service_admin(&self, id: i32) -> Result<AdminServiceDto> {
        self.service_repo
            .find_by_id_admin(id)
            .await?
            .map(to_service_dto)
            .ok_or_else(|| Error::new(ErrorCode::ServiceNotFound))
    }

    pub async fn create_service_admin(
        &self,
        actor: &Actor,
        business_id: i32,
        category_id: i32,
        name: String,
        description: Option<String>,
        price_note: Option<String>,
    ) -> Result<AdminServiceDto> {
        actor.require(PERM_SERVICE_MANAGE)?;
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(Error::new(ErrorCode::NameRequired));
        }
        let biz_exists = self
            .business_repo
            .check_exists_for_admin(business_id)
            .await?;
        if !biz_exists {
            return Err(Error::new(ErrorCode::ProviderNotFound));
        }
        let valid_cat = self
            .service_repo
            .check_category_is_service(category_id)
            .await?;
        if !valid_cat {
            return Err(Error::new(ErrorCode::InvalidServiceCategory));
        }
        let id = self
            .service_repo
            .create(NewService {
                business_id,
                category_id,
                name,
                description,
                price_note,
                created_by: actor.user_id,
            })
            .await?;
        self.load_service_admin(id).await
    }

    pub async fn update_service_admin(
        &self,
        actor: &Actor,
        id: i32,
        category_id: i32,
        name: String,
        description: Option<String>,
        price_note: Option<String>,
    ) -> Result<AdminServiceDto> {
        actor.require(PERM_SERVICE_MANAGE)?;
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(Error::new(ErrorCode::NameRequired));
        }
        let valid_cat = self
            .service_repo
            .check_category_is_service(category_id)
            .await?;
        if !valid_cat {
            return Err(Error::new(ErrorCode::InvalidServiceCategory));
        }
        let updated = self
            .service_repo
            .update(UpdateService {
                id,
                category_id,
                name,
                description,
                price_note,
                updated_by: actor.user_id,
            })
            .await?;
        if !updated {
            return Err(Error::new(ErrorCode::ServiceNotFound));
        }
        self.load_service_admin(id).await
    }

    pub async fn toggle_service_active(
        &self,
        actor: &Actor,
        id: i32,
        is_active: bool,
        reason: Option<String>,
    ) -> Result<AdminServiceDto> {
        actor.require(PERM_SERVICE_MANAGE)?;
        let updated = self.service_repo.toggle_active(id, is_active).await?;
        if !updated {
            return Err(Error::new(ErrorCode::ServiceNotFound));
        }
        self.publish(
            actor,
            Entity::Service,
            id,
            Change::Visibility { visible: is_active },
            reason,
        )
        .await;
        self.load_service_admin(id).await
    }

    pub async fn delete_service_admin(
        &self,
        actor: &Actor,
        id: i32,
        reason: Option<String>,
    ) -> Result<()> {
        actor.require(PERM_SERVICE_MANAGE)?;
        let deleted = self.service_repo.delete(id).await?;
        if !deleted {
            return Err(Error::new(ErrorCode::ServiceNotFound));
        }
        self.publish(actor, Entity::Service, id, Change::Deleted, reason)
            .await;
        Ok(())
    }

    // ── Roles (read-only) ─────────────────────────────────────────────────────

    pub async fn list_roles(&self, actor: &Actor) -> Result<Vec<RoleDto>> {
        actor.require(PERM_ROLE_READ)?;
        self.role_repo
            .list()
            .await
            .map(|roles| roles.into_iter().map(RoleDto::from).collect())
    }

    /// The roles a user can be moved to: for whoever may change user roles,
    /// even without permission to browse the role pages.
    pub async fn assignable_roles(&self, actor: &Actor) -> Result<Vec<RoleDto>> {
        actor.require(PERM_USER_ROLE_UPDATE)?;
        Ok(self
            .role_repo
            .list()
            .await?
            .into_iter()
            .filter(|r| r.is_active)
            .map(RoleDto::from)
            .collect())
    }

    pub async fn get_role_by_id(&self, actor: &Actor, id: i32) -> Result<RoleDto> {
        actor.require(PERM_ROLE_READ)?;
        self.role_repo
            .find_by_id(id)
            .await?
            .map(RoleDto::from)
            .ok_or_else(|| Error::new(ErrorCode::RoleNotFound))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn to_location_dto(l: Location) -> AdminLocationDto {
    AdminLocationDto {
        id: l.id,
        label: l.label,
        lat: l.lat,
        lng: l.lng,
        display_order: l.display_order,
        is_active: l.is_active,
    }
}

fn to_service_dto(s: AdminService) -> AdminServiceDto {
    AdminServiceDto {
        id: s.id,
        name: s.name,
        description: s.description,
        price_note: s.price_note,
        category: s.category,
        category_id: s.category_id,
        business_id: s.business_id,
        business_name: s.business_name,
        is_active: s.is_active,
        created_on: s.created_on,
        updated_on: s.updated_on,
    }
}

fn to_category_dto(c: CategoryAdmin) -> AdminCategoryDto {
    AdminCategoryDto {
        id: c.id,
        name: c.name,
        slug: c.slug,
        category_type: c.category_type,
        icon: c.icon,
        color: c.color,
        description: c.description,
        display_order: c.display_order,
        is_active: c.is_active,
        created_on: c.created_on,
        updated_on: c.updated_on,
        created_by_name: c.created_by_name,
        updated_by_name: c.updated_by_name,
    }
}

/// Trims every field and rejects a blank name or slug, reporting both at once.
/// The trimmed label of a valid location preset; every bad field reported.
fn validate_location(label: &str, lat: f64, lng: f64) -> Result<String> {
    let label = label.trim().to_owned();
    Validation::new()
        .check(label.is_empty(), "label", ErrorCode::LabelRequired)
        .rule("label", max_chars(&label, LOCATION_LABEL_MAX))
        .rule("lat", validate_coordinates(lat, lng))
        .finish()?;
    Ok(label)
}

/// `category_type` is checked on create only: a category never changes type.
fn validate_category(input: CategoryInput, category_type: Option<&str>) -> Result<CategoryInput> {
    let name = input.name.trim().to_owned();
    let slug = input.slug.trim().to_owned();
    let icon = non_blank(input.icon);
    let color = non_blank(input.color);
    let description = non_blank(input.description);
    let opt = |v: &Option<String>, max| v.as_deref().map_or(Ok(()), |v| max_chars(v, max));
    Validation::new()
        .check(name.is_empty(), "name", ErrorCode::NameRequired)
        .rule("name", max_chars(&name, CATEGORY_NAME_MAX))
        .check(slug.is_empty(), "slug", ErrorCode::SlugRequired)
        .rule("slug", validate_slug(&slug))
        .rule("slug", max_chars(&slug, CATEGORY_SLUG_MAX))
        .rule("icon", opt(&icon, CATEGORY_ICON_MAX))
        .rule("color", opt(&color, CATEGORY_COLOR_MAX))
        .rule("description", opt(&description, CATEGORY_DESCRIPTION_MAX))
        .check(
            category_type.is_some_and(|t| !CATEGORY_TYPES.contains(&t)),
            "category_type",
            ErrorCode::InvalidCategoryType,
        )
        .finish()?;
    Ok(CategoryInput {
        name,
        slug,
        icon,
        color,
        description,
        display_order: input.display_order,
    })
}

/// An admin who demotes or deactivates themselves can lock the last admin out
/// of the panel, with nobody left to undo it.
fn refuse_self(actor: &Actor, target_user_id: i32) -> Result<()> {
    if actor.user_id == target_user_id {
        return Err(Error::new(ErrorCode::CannotChangeOwnAccount));
    }
    Ok(())
}
