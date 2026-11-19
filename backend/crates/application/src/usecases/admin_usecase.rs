use std::sync::Arc;

use chrono::{Duration, Utc};

use domain::{
    constants::{
        CATEGORY_TYPES, INQUIRY_STATUSES, INQUIRY_STATUS_NEW, INQUIRY_STATUS_SPAM, ROLE_PROVIDER,
    },
    events::{Change, DomainEvent, Entity},
    models::{
        category::{CategoryAdmin, NewCategory, UpdateCategory},
        coded::CodedEntry,
        location::Location,
        product::{AdminProductFilter, NewProduct, UpdateProductAdmin},
        role::UserRole,
        service::{NewService, UpdateService},
    },
    repositories::{
        admin_unit_repository_trait::AdminUnitRepositoryTrait,
        audit_log_repository_trait::AuditLogRepositoryTrait,
        business_repository_trait::BusinessRepositoryTrait,
        call_repository_trait::CallRepositoryTrait,
        category_repository_trait::CategoryRepositoryTrait,
        coded_repository_trait::CodedRepositoryTrait,
        inquiry_repository_trait::InquiryRepositoryTrait,
        location_repository_trait::LocationRepositoryTrait,
        product_repository_trait::ProductRepositoryTrait,
        refresh_token_repository_trait::RefreshTokenRepositoryTrait,
        review_repository_trait::ReviewRepositoryTrait, role_repository_trait::RoleRepositoryTrait,
        service_repository_trait::ServiceRepositoryTrait,
        user_repository_trait::UserRepositoryTrait,
    },
    Error, ErrorCode, Result, Validation,
};

use crate::{
    admin_units::{preset_label, AdminUnits, AreaKey},
    authz::Actor,
    catalogue::{
        PERM_CATEGORY_MANAGE, PERM_INQUIRY_MANAGE, PERM_INQUIRY_READ, PERM_LISTING_MODERATE,
        PERM_LOCATION_MANAGE, PERM_PROVIDER_ACCOUNT_ASSIGN, PERM_PROVIDER_MANAGE,
        PERM_PROVIDER_READ, PERM_PROVIDER_STATUS_UPDATE, PERM_PROVIDER_VERIFY,
        PERM_REVIEW_MODERATE, PERM_ROLE_READ, PERM_SERVICE_MANAGE, PERM_USER_READ,
        PERM_USER_ROLE_UPDATE, PERM_USER_STATUS_UPDATE,
    },
    constants::{
        ADMIN_PAGE_SIZE, CALL_STATS_DAYS, CATEGORY_COLOR_MAX, CATEGORY_DESCRIPTION_MAX,
        CATEGORY_ICON_MAX, CATEGORY_NAME_MAX, CATEGORY_SLUG_MAX, HISTORY_SHOWN, LOCATION_LABEL_MAX,
        SLUG_CANDIDATES_PER_QUERY,
    },
    inputs::{
        CategoryInput, CreateProductInput, LocationInput, UpdateProductInput, UpdateProfileInput,
    },
    outputs::{
        AdminCategoryDto, AdminInquiryDto, AdminLocationDto, AdminPagedResponse, AdminProductDto,
        AdminProductItemDto, AdminProviderDetailDto, AdminProviderDto, AdminReviewDto,
        AdminServiceDto, AdminServiceItemDto, AdminUserDetailDto, AdminUserDto, AreaGroupDto,
        AssignUserDto, AuditEntryDto, InquiryCountsDto, ProviderActivityDto, ProviderStatusDto,
        ReviewCountsDto, RoleDto, SlugSuggestionDto, VerificationDto,
    },
    pagination::{admin_total_pages, paginate_admin, total_or_count},
    ports::{EventBus, PasswordHasher},
    settings::SettingsReader,
    slug::{numbered, slugify},
    usecases::{
        account_rules::account_for_role,
        business_rules::validate_profile,
        product_rules::{validate_product, ProductFields},
        review_usecase::one_decimal,
        service_rules::{validate_service, ServiceFields},
    },
    validation::{
        self, audit_reason, max_chars, max_chars_opt, non_blank, validate_coordinates,
        validate_slug,
    },
};

// ── Use case ──────────────────────────────────────────────────────────────────

/// A record whose changes an admin page lists (`AdminUseCase::history`).
#[derive(Debug, Clone, Copy)]
pub enum HistoryOf {
    Provider(i32),
    Service(i32),
    Product(i32),
}

impl HistoryOf {
    /// What its own admin page needs: the same to read its history.
    fn permission(self) -> &'static str {
        match self {
            Self::Provider(_) => PERM_PROVIDER_READ,
            Self::Service(_) => PERM_SERVICE_MANAGE,
            Self::Product(_) => PERM_LISTING_MODERATE,
        }
    }

    /// The record as the audit log names it.
    fn record(self) -> (Entity, i32) {
        match self {
            Self::Provider(id) => (Entity::Provider, id),
            Self::Service(id) => (Entity::Service, id),
            Self::Product(id) => (Entity::Product, id),
        }
    }
}

#[derive(Clone)]
pub struct AdminUseCase {
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    /// The wards a new preset may be filled from.
    admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    review_repo: Arc<dyn ReviewRepositoryTrait + Send + Sync>,
    product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    role_repo: Arc<dyn RoleRepositoryTrait + Send + Sync>,
    service_repo: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
    /// The sectors a type may serve.
    sector_repo: Arc<dyn CodedRepositoryTrait + Send + Sync>,
    events: Arc<dyn EventBus>,
    /// Password hashing injected via port — infrastructure stays out of use cases.
    password_hasher: Arc<dyn PasswordHasher>,
    /// A deactivated account's sessions end with it.
    refresh_repo: Arc<dyn RefreshTokenRepositoryTrait + Send + Sync>,
    /// A provider's call and Zalo taps, for its page.
    call_repo: Arc<dyn CallRepositoryTrait + Send + Sync>,
    /// A record's latest changes, for its page.
    audit_repo: Arc<dyn AuditLogRepositoryTrait + Send + Sync>,
    /// How soon after its request a review is flagged.
    settings: SettingsReader,
}

/// Everything `AdminUseCase` depends on, named so wiring cannot swap two repos
/// of compatible types by position.
pub struct AdminDeps {
    pub business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    pub user_repo: Arc<dyn UserRepositoryTrait + Send + Sync>,
    pub category_repo: Arc<dyn CategoryRepositoryTrait + Send + Sync>,
    pub inquiry_repo: Arc<dyn InquiryRepositoryTrait + Send + Sync>,
    pub location_repo: Arc<dyn LocationRepositoryTrait + Send + Sync>,
    pub admin_unit_repo: Arc<dyn AdminUnitRepositoryTrait + Send + Sync>,
    pub review_repo: Arc<dyn ReviewRepositoryTrait + Send + Sync>,
    pub product_repo: Arc<dyn ProductRepositoryTrait + Send + Sync>,
    pub role_repo: Arc<dyn RoleRepositoryTrait + Send + Sync>,
    pub service_repo: Arc<dyn ServiceRepositoryTrait + Send + Sync>,
    pub sector_repo: Arc<dyn CodedRepositoryTrait + Send + Sync>,
    pub events: Arc<dyn EventBus>,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub refresh_repo: Arc<dyn RefreshTokenRepositoryTrait + Send + Sync>,
    pub call_repo: Arc<dyn CallRepositoryTrait + Send + Sync>,
    pub audit_repo: Arc<dyn AuditLogRepositoryTrait + Send + Sync>,
    pub settings: SettingsReader,
}

impl AdminUseCase {
    pub fn new(d: AdminDeps) -> Self {
        Self {
            business_repo: d.business_repo,
            user_repo: d.user_repo,
            category_repo: d.category_repo,
            inquiry_repo: d.inquiry_repo,
            location_repo: d.location_repo,
            admin_unit_repo: d.admin_unit_repo,
            review_repo: d.review_repo,
            product_repo: d.product_repo,
            role_repo: d.role_repo,
            service_repo: d.service_repo,
            sector_repo: d.sector_repo,
            events: d.events,
            password_hasher: d.password_hasher,
            refresh_repo: d.refresh_repo,
            call_repo: d.call_repo,
            audit_repo: d.audit_repo,
            settings: d.settings,
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
                actor_id: Some(actor.user_id),
                entity,
                entity_id,
                change,
                reason: audit_reason(reason),
            })
            .await;
    }

    /// The latest `HISTORY_SHOWN` changes to one record, newest first: who
    /// hid it, verified it, edited it, and why. Read with the permission its
    /// own admin page needs.
    pub async fn history(&self, actor: &Actor, of: HistoryOf) -> Result<Vec<AuditEntryDto>> {
        actor.require(of.permission())?;
        let (entity, id) = of.record();
        Ok(self
            .audit_repo
            .recent(entity.as_str(), id, HISTORY_SHOWN)
            .await?
            .into_iter()
            .map(AuditEntryDto::from)
            .collect())
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
        let activity = self.provider_activity(actor, id).await?;
        let area = match (&b.province_name, &b.ward_name) {
            (Some(province), Some(ward)) => Some(preset_label(province, ward)),
            _ => None,
        };

        Ok(AdminProviderDetailDto {
            id: b.id,
            name: b.name,
            phone: b.phone,
            email: b.email,
            description: b.description,
            address: b.address,
            is_verified: b.is_verified,
            is_active: b.is_active,
            is_demo: b.is_demo,
            area,
            activity,
            created_on: b.created_on,
            linked_user_id: b.linked_user_id,
            linked_username: b.linked_username,
            lat: b.lat,
            lng: b.lng,
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

    /// One provider's recent leads and reviews: taps over `CALL_STATS_DAYS`
    /// (as its owner's dashboard counts them), contact requests and reviews
    /// only for a visitor who may read those lists.
    async fn provider_activity(&self, actor: &Actor, id: i32) -> Result<ProviderActivityDto> {
        let since = Utc::now() - Duration::days(CALL_STATS_DAYS);
        let taps = self.call_repo.count_since(Some(id), since).await?;
        let inquiries = if actor.can(PERM_INQUIRY_READ) {
            let count = |status| self.inquiry_repo.count_admin(Some(id), status);
            let all = count(None).await?;
            let spam = count(Some(INQUIRY_STATUS_SPAM)).await?;
            Some(InquiryCountsDto {
                waiting: count(Some(INQUIRY_STATUS_NEW)).await?,
                total: all - spam,
            })
        } else {
            None
        };
        let reviews = if actor.can(PERM_REVIEW_MODERATE) {
            let all = self.review_repo.count_admin(Some(id)).await?;
            let shown = self.review_repo.summary_public(id).await?;
            Some(ReviewCountsDto {
                shown: shown.count,
                average: shown.average.map(one_decimal),
                waiting: all - shown.count,
            })
        } else {
            None
        };
        Ok(ProviderActivityDto {
            taps,
            days: CALL_STATS_DAYS,
            inquiries,
            reviews,
        })
    }

    pub async fn list_providers(
        &self,
        actor: &Actor,
        page: i64,
        search: Option<&str>,
    ) -> Result<AdminPagedResponse<AdminProviderDto>> {
        actor.require(PERM_PROVIDER_READ)?;
        let (page, offset) = paginate_admin(page);
        let items = self
            .business_repo
            .list_for_admin(search, ADMIN_PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, items.len(), ADMIN_PAGE_SIZE, || {
            self.business_repo.count_for_admin(search)
        })
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
            total_pages: admin_total_pages(total),
        })
    }

    /// Grants or withdraws the "verified" badge. Setting the state it already
    /// has changes nothing and is not audited, so a form sent twice is harmless.
    pub async fn set_verification(
        &self,
        actor: &Actor,
        id: i32,
        is_verified: bool,
        reason: Option<String>,
    ) -> Result<VerificationDto> {
        actor.require(PERM_PROVIDER_VERIFY)?;
        let (name, changed) = self
            .business_repo
            .set_verified(id, is_verified)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ProviderNotFound))?;
        if changed {
            self.publish(
                actor,
                Entity::Provider,
                id,
                Change::Verification {
                    verified: is_verified,
                },
                reason,
            )
            .await;
        }
        Ok(VerificationDto {
            id,
            name,
            is_verified,
        })
    }

    /// Turns a provider off (gone from every public page with its services
    /// and products; its account can still sign in) or on again. Setting the
    /// state it already has changes nothing and is not audited.
    pub async fn toggle_provider_active(
        &self,
        actor: &Actor,
        id: i32,
        is_active: bool,
        reason: Option<String>,
    ) -> Result<ProviderStatusDto> {
        actor.require(PERM_PROVIDER_STATUS_UPDATE)?;
        let (name, changed) = self
            .business_repo
            .set_active(id, is_active, actor.user_id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ProviderNotFound))?;
        if changed {
            self.publish(
                actor,
                Entity::Provider,
                id,
                Change::Visibility { visible: is_active },
                reason,
            )
            .await;
        }
        Ok(ProviderStatusDto {
            id,
            name,
            is_active,
        })
    }

    /// Lists a business with no account yet: the admin on a field visit sets
    /// it up from the owner's phone number, and an account can be assigned
    /// later. Its id.
    pub async fn create_provider(&self, actor: &Actor, input: UpdateProfileInput) -> Result<i32> {
        actor.require(PERM_PROVIDER_MANAGE)?;
        let profile = validate_profile(input)?;
        let id = self.business_repo.create(&profile, actor.user_id).await?;
        self.publish(actor, Entity::Provider, id, Change::Created, None)
            .await;
        Ok(id)
    }

    /// Replaces a business's profile, as its owner's own form would.
    pub async fn update_provider(
        &self,
        actor: &Actor,
        id: i32,
        input: UpdateProfileInput,
    ) -> Result<()> {
        actor.require(PERM_PROVIDER_MANAGE)?;
        let profile = validate_profile(input)?;
        if !self
            .business_repo
            .update_profile_by_id(id, &profile, actor.user_id)
            .await?
        {
            return Err(Error::new(ErrorCode::ProviderNotFound));
        }
        self.publish(actor, Entity::Provider, id, Change::Updated, None)
            .await;
        Ok(())
    }

    /// Gives a business an account to manage it from the provider dashboard.
    /// An existing provider account is linked as-is (its email and password
    /// are left alone); otherwise a provider account is created with the
    /// given email and password. Linking replaces the business's previous
    /// account, if any.
    pub async fn assign_provider_user(
        &self,
        actor: &Actor,
        business_id: i32,
        username: String,
        email: String,
        password: String,
    ) -> Result<AssignUserDto> {
        actor.require(PERM_PROVIDER_ACCOUNT_ASSIGN)?;
        if !self
            .business_repo
            .check_exists_for_admin(business_id)
            .await?
        {
            return Err(Error::new(ErrorCode::ProviderNotFound));
        }
        // One account manages one business: the dashboard shows the business
        // its account owns. A new account owns none.
        if let Some(u) = self
            .user_repo
            .find_by_username(&username.trim().to_lowercase())
            .await?
        {
            let owned = self.business_repo.get_profile_by_user(u.id).await?;
            if u.role == ROLE_PROVIDER && owned.is_some_and(|b| b.id != business_id) {
                return Err(Error::field("username", ErrorCode::ProviderAccountInUse));
            }
        }
        let (user_id, username) = account_for_role(
            self.user_repo.as_ref(),
            self.password_hasher.as_ref(),
            &username,
            &email,
            &password,
            ROLE_PROVIDER,
        )
        .await?;

        self.business_repo.assign_user(business_id, user_id).await?;
        self.publish(
            actor,
            Entity::Provider,
            business_id,
            Change::AccountAssigned {
                username: username.clone(),
            },
            None,
        )
        .await;

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
        let business_id = self.business_repo.get_business_id_by_user(u.id).await?;

        Ok(AdminUserDetailDto {
            business_id,
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
        search: Option<&str>,
    ) -> Result<AdminPagedResponse<AdminUserDto>> {
        actor.require(PERM_USER_READ)?;
        let (page, offset) = paginate_admin(page);
        let items = self
            .user_repo
            .list_for_admin(search, ADMIN_PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, items.len(), ADMIN_PAGE_SIZE, || {
            self.user_repo.count_for_admin(search)
        })
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
            total_pages: admin_total_pages(total),
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
        if !is_active {
            // Its sessions end for good: turned on again, the account logs
            // in again, on no device that was signed in before.
            self.user_repo.end_sessions(user_id).await?;
            self.refresh_repo.revoke_user(user_id).await?;
        }
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
        let known = self.sector_repo.list_all().await?;
        let input = validate_category(input, Some(&category_type), &known)?;
        let slug = self.settle_category_slug(&input, None).await?;
        let id = self
            .category_repo
            .create(NewCategory {
                name: input.name,
                slug,
                category_type,
                icon: input.icon,
                color: input.color,
                description: input.description,
                display_order: input.display_order,
                sector: sector_of(&input.sector),
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
        let known = self.sector_repo.list_all().await?;
        let input = validate_category(input, None, &known)?;
        let slug = self.settle_category_slug(&input, Some(id)).await?;
        self.category_repo
            .update(
                id,
                UpdateCategory {
                    name: input.name,
                    slug,
                    icon: input.icon,
                    color: input.color,
                    description: input.description,
                    display_order: input.display_order,
                    sector: sector_of(&input.sector),
                    updated_by: actor.user_id,
                },
            )
            .await?;
        self.publish(actor, Entity::Category, id, Change::Updated, None)
            .await;
        self.load_category_detail(id).await
    }

    /// What the category form's slug box offers for `text` (the name as
    /// typed, or a slug): its slug, numbered while another category has it.
    /// `exclude_id`: the category being edited, whose own slug is free to it.
    pub async fn suggest_category_slug(
        &self,
        actor: &Actor,
        text: &str,
        exclude_id: Option<i32>,
    ) -> Result<SlugSuggestionDto> {
        actor.require(PERM_CATEGORY_MANAGE)?;
        let base = numbered(&slugify(text), 1, CATEGORY_SLUG_MAX);
        if base.is_empty() {
            return Ok(SlugSuggestionDto {
                slug: base,
                taken: false,
            });
        }
        let slug = self.free_category_slug(&base, exclude_id).await?;
        Ok(SlugSuggestionDto {
            taken: slug != base,
            slug,
        })
    }

    /// The slug a save stores: the typed one unless another category has it
    /// (`slug_taken`), else one made from the name, numbered until free. The
    /// UNIQUE constraint still decides a race between two saves.
    async fn settle_category_slug(
        &self,
        input: &CategoryInput,
        exclude_id: Option<i32>,
    ) -> Result<String> {
        if input.slug.is_empty() {
            let base = numbered(&slugify(&input.name), 1, CATEGORY_SLUG_MAX);
            return self.free_category_slug(&base, exclude_id).await;
        }
        let taken = self
            .category_repo
            .taken_slugs(std::slice::from_ref(&input.slug), exclude_id)
            .await?;
        if taken.is_empty() {
            Ok(input.slug.clone())
        } else {
            Err(Error::field("slug", ErrorCode::SlugTaken))
        }
    }

    /// `base` when no other category has it, else the first free of
    /// `base-2`, `base-3`, …, asked for a few at a time.
    async fn free_category_slug(&self, base: &str, exclude_id: Option<i32>) -> Result<String> {
        let mut first = 1;
        loop {
            let candidates: Vec<String> = (first..first + SLUG_CANDIDATES_PER_QUERY)
                .map(|n| numbered(base, n, CATEGORY_SLUG_MAX))
                .collect();
            let taken = self
                .category_repo
                .taken_slugs(&candidates, exclude_id)
                .await?;
            if let Some(free) = candidates.into_iter().find(|c| !taken.contains(c)) {
                return Ok(free);
            }
            first += SLUG_CANDIDATES_PER_QUERY;
        }
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

    /// `provider_id`: only that provider's inquiries; `status`: only those
    /// in it (one of `INQUIRY_STATUSES`).
    pub async fn list_inquiries(
        &self,
        actor: &Actor,
        page: i64,
        provider_id: Option<i32>,
        status: Option<&str>,
    ) -> Result<AdminPagedResponse<AdminInquiryDto>> {
        actor.require(PERM_INQUIRY_READ)?;
        if status.is_some_and(|s| !INQUIRY_STATUSES.contains(&s)) {
            return Err(Error::field("status", ErrorCode::InvalidInquiryStatus));
        }
        let (page, offset) = paginate_admin(page);
        let items = self
            .inquiry_repo
            .list_admin(provider_id, status, ADMIN_PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, items.len(), ADMIN_PAGE_SIZE, || {
            self.inquiry_repo.count_admin(provider_id, status)
        })
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
            total_pages: admin_total_pages(total),
        })
    }

    /// How many requests are in `status` (one provider's when `provider_id`
    /// is set): the count the list's chip of that status shows.
    pub async fn count_inquiries(
        &self,
        actor: &Actor,
        provider_id: Option<i32>,
        status: &str,
    ) -> Result<i64> {
        actor.require(PERM_INQUIRY_READ)?;
        if !INQUIRY_STATUSES.contains(&status) {
            return Err(Error::field("status", ErrorCode::InvalidInquiryStatus));
        }
        self.inquiry_repo
            .count_admin(provider_id, Some(status))
            .await
    }

    /// `reason`: why, for the audit log (marking spam asks for one).
    pub async fn update_inquiry_status(
        &self,
        actor: &Actor,
        id: i32,
        status: String,
        reason: Option<String>,
    ) -> Result<AdminInquiryDto> {
        actor.require(PERM_INQUIRY_MANAGE)?;
        if !INQUIRY_STATUSES.contains(&status.as_str()) {
            return Err(Error::new(ErrorCode::InvalidInquiryStatus));
        }
        self.inquiry_repo
            .update_status(id, &status, actor.user_id)
            .await?;
        self.publish(
            actor,
            Entity::Inquiry,
            id,
            Change::StatusChanged {
                status: status.clone(),
            },
            reason,
        )
        .await;
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

    /// `provider_id`: only that provider's reviews.
    pub async fn list_reviews(
        &self,
        actor: &Actor,
        page: i64,
        provider_id: Option<i32>,
    ) -> Result<AdminPagedResponse<AdminReviewDto>> {
        actor.require(PERM_REVIEW_MODERATE)?;
        let suspect_minutes = self.settings.load().await?.review_suspect_minutes();
        let (page, offset) = paginate_admin(page);
        let items = self
            .review_repo
            .list_admin(provider_id, ADMIN_PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, items.len(), ADMIN_PAGE_SIZE, || {
            self.review_repo.count_admin(provider_id)
        })
        .await?;

        Ok(AdminPagedResponse {
            items: items
                .into_iter()
                .map(|r| AdminReviewDto::new(r, suspect_minutes))
                .collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: admin_total_pages(total),
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
        let suspect_minutes = self.settings.load().await?.review_suspect_minutes();
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
        self.review_repo
            .find_by_id_admin(id)
            .await?
            .map(|r| AdminReviewDto::new(r, suspect_minutes))
            .ok_or_else(|| Error::new(ErrorCode::ReviewNotFound))
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

    /// The wards a new preset may be filled from, by province.
    pub async fn location_choices(&self, actor: &Actor) -> Result<Vec<AreaGroupDto>> {
        actor.require(PERM_LOCATION_MANAGE)?;
        Ok(AdminUnits::load(self.admin_unit_repo.as_ref())
            .await?
            .choices())
    }

    /// A preset's boxes, checked. A ward chosen fills what was left blank:
    /// the label with its preset name, a missing coordinate with its point;
    /// what was typed stays. An unknown or turned-off ward is a field error.
    /// The one rule of the create and the edit forms (and their JSON).
    async fn location_fields(&self, input: LocationInput) -> Result<(String, f64, f64, i32)> {
        let LocationInput {
            mut label,
            mut lat,
            mut lng,
            display_order,
            ward,
        } = input;
        if let Some(ward) = ward.as_deref().map(str::trim).filter(|w| !w.is_empty()) {
            let units = AdminUnits::load(self.admin_unit_repo.as_ref()).await?;
            let found = match AreaKey::parse(ward) {
                Some(AreaKey::Ward(code)) => units
                    .ward(&code)
                    .filter(|w| w.is_active)
                    .and_then(|w| units.province(&w.province_code).map(|p| (p, w))),
                _ => None,
            };
            let Some((province, ward)) = found else {
                return Err(Error::field("ward", ErrorCode::LocationNotFound));
            };
            if label.trim().is_empty() {
                label = preset_label(&province.name, &ward.name);
            }
            if !lat.is_finite() || !lng.is_finite() {
                (lat, lng) = (ward.lat, ward.lng);
            }
        }
        let label = validate_location(&label, lat, lng)?;
        Ok((label, lat, lng, display_order))
    }

    pub async fn create_location(
        &self,
        actor: &Actor,
        input: LocationInput,
    ) -> Result<AdminLocationDto> {
        actor.require(PERM_LOCATION_MANAGE)?;
        let (label, lat, lng, display_order) = self.location_fields(input).await?;
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
        input: LocationInput,
    ) -> Result<AdminLocationDto> {
        actor.require(PERM_LOCATION_MANAGE)?;
        let (label, lat, lng, display_order) = self.location_fields(input).await?;
        self.location_repo
            .update(id, label, lat, lng, display_order)
            .await?;
        self.publish(actor, Entity::Location, id, Change::Updated, None)
            .await;
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
        filter: &AdminProductFilter,
    ) -> Result<AdminPagedResponse<AdminProductDto>> {
        actor.require(PERM_LISTING_MODERATE)?;
        let (page, offset) = paginate_admin(page);
        let items = self
            .product_repo
            .list_admin(filter, ADMIN_PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, items.len(), ADMIN_PAGE_SIZE, || {
            self.product_repo.count_admin(filter)
        })
        .await?;
        Ok(AdminPagedResponse {
            items: items.into_iter().map(AdminProductDto::from).collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: admin_total_pages(total),
        })
    }

    pub async fn get_product_admin(&self, actor: &Actor, id: i32) -> Result<AdminProductDto> {
        actor.require(PERM_LISTING_MODERATE)?;
        self.load_product_admin(id).await
    }

    /// Lists a supply for a provider, e.g. one with no account met on a
    /// field visit: the owner's rules (`product_rules`) and cap; its photo is
    /// added on its page (`MediaUseCase::attach_product_image_admin`).
    pub async fn create_product_admin(
        &self,
        actor: &Actor,
        business_id: i32,
        input: CreateProductInput,
    ) -> Result<AdminProductDto> {
        actor.require(PERM_LISTING_MODERATE)?;
        if !self
            .business_repo
            .check_exists_for_admin(business_id)
            .await?
        {
            return Err(Error::new(ErrorCode::ProviderNotFound));
        }
        let CreateProductInput {
            category_id,
            commodity_id,
            name,
            description,
            price,
            unit,
        } = input;
        let ProductFields {
            name,
            description,
            unit,
            commodity_id,
        } = validate_product(
            self.product_repo.as_ref(),
            category_id,
            commodity_id,
            name,
            description,
            price,
            unit,
        )
        .await?;
        let cap = self.settings.load().await?.products_per_business();
        let id = self
            .product_repo
            .create_capped(
                NewProduct {
                    business_id,
                    category_id,
                    commodity_id,
                    name,
                    description,
                    price,
                    unit,
                    image_file_id: None,
                    created_by: actor.user_id,
                },
                cap,
            )
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ProductLimitReached).with_param("max", cap))?;
        self.publish(actor, Entity::Product, id, Change::Created, None)
            .await;
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

    /// An admin's edit of any product: the same rules as the owner's form
    /// (`product_rules`); its visibility and photo stay as they are.
    pub async fn update_product_admin(
        &self,
        actor: &Actor,
        input: UpdateProductInput,
    ) -> Result<AdminProductDto> {
        actor.require(PERM_LISTING_MODERATE)?;
        let UpdateProductInput {
            product_id: id,
            category_id,
            commodity_id,
            name,
            description,
            price,
            unit,
        } = input;
        let ProductFields {
            name,
            description,
            unit,
            commodity_id,
        } = validate_product(
            self.product_repo.as_ref(),
            category_id,
            commodity_id,
            name,
            description,
            price,
            unit,
        )
        .await?;
        let updated = self
            .product_repo
            .update_admin(UpdateProductAdmin {
                id,
                category_id,
                commodity_id,
                name,
                description,
                price,
                unit,
                updated_by: actor.user_id,
            })
            .await?;
        if !updated {
            return Err(Error::new(ErrorCode::ProductNotFound));
        }
        self.publish(actor, Entity::Product, id, Change::Updated, None)
            .await;
        self.load_product_admin(id).await
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
        search: Option<&str>,
    ) -> Result<AdminPagedResponse<AdminServiceDto>> {
        actor.require(PERM_SERVICE_MANAGE)?;
        let (page, offset) = paginate_admin(page);
        let items = self
            .service_repo
            .list_admin(provider_id, search, ADMIN_PAGE_SIZE, offset)
            .await?;
        let total = total_or_count(offset, items.len(), ADMIN_PAGE_SIZE, || {
            self.service_repo.count_admin(provider_id, search)
        })
        .await?;
        Ok(AdminPagedResponse {
            items: items.into_iter().map(AdminServiceDto::from).collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: admin_total_pages(total),
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
            .map(AdminServiceDto::from)
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
        if !self
            .business_repo
            .check_exists_for_admin(business_id)
            .await?
        {
            return Err(Error::new(ErrorCode::ProviderNotFound));
        }
        let ServiceFields {
            name,
            description,
            price_note,
        } = validate_service(
            self.service_repo.as_ref(),
            category_id,
            name,
            description,
            price_note,
        )
        .await?;
        // The owner's cap holds for an admin too: an owner's page shows
        // their whole list at once.
        let cap = self.settings.load().await?.services_per_business();
        let id = self
            .service_repo
            .create_capped(
                NewService {
                    business_id,
                    category_id,
                    name,
                    description,
                    price_note,
                    image_file_id: None,
                    created_by: actor.user_id,
                },
                cap,
            )
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ServiceLimitReached).with_param("max", cap))?;
        self.publish(actor, Entity::Service, id, Change::Created, None)
            .await;
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
        let ServiceFields {
            name,
            description,
            price_note,
        } = validate_service(
            self.service_repo.as_ref(),
            category_id,
            name,
            description,
            price_note,
        )
        .await?;
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
        self.publish(actor, Entity::Service, id, Change::Updated, None)
            .await;
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
        is_demo: l.is_demo,
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
        sector: c.sector,
        is_active: c.is_active,
        is_demo: c.is_demo,
        created_on: c.created_on,
        updated_on: c.updated_on,
        created_by_name: c.created_by_name,
        updated_by_name: c.updated_by_name,
    }
}

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

/// A validated sector code as stored: none for every sector.
fn sector_of(sector: &str) -> Option<String> {
    (!sector.is_empty()).then(|| sector.to_owned())
}

/// Trims every field and reports every bad one at once. A blank slug is made
/// from the name on save, so it is refused only when the name gives none.
/// `category_type` is checked on create only: a category never changes type.
fn validate_category(
    input: CategoryInput,
    category_type: Option<&str>,
    known: &[CodedEntry],
) -> Result<CategoryInput> {
    let name = input.name.trim().to_owned();
    let slug = input.slug.trim().to_owned();
    let icon = non_blank(input.icon);
    let color = non_blank(input.color);
    let description = non_blank(input.description);
    let sector = input.sector.trim().to_owned();
    Validation::new()
        .check(name.is_empty(), "name", ErrorCode::NameRequired)
        .rule("name", max_chars(&name, CATEGORY_NAME_MAX))
        .check(
            slug.is_empty() && slugify(&name).is_empty(),
            "slug",
            ErrorCode::SlugRequired,
        )
        .rule(
            "slug",
            if slug.is_empty() {
                Ok(())
            } else {
                validate_slug(&slug)
            },
        )
        .rule("slug", max_chars(&slug, CATEGORY_SLUG_MAX))
        .rule("icon", max_chars_opt(&icon, CATEGORY_ICON_MAX))
        .rule("color", max_chars_opt(&color, CATEGORY_COLOR_MAX))
        .rule(
            "description",
            max_chars_opt(&description, CATEGORY_DESCRIPTION_MAX),
        )
        .check(
            category_type.is_some_and(|t| !CATEGORY_TYPES.contains(&t)),
            "category_type",
            ErrorCode::InvalidCategoryType,
        )
        .check(
            validation::sector(&sector, known).is_err(),
            "sector",
            ErrorCode::InvalidSector,
        )
        .finish()?;
    Ok(CategoryInput {
        name,
        slug,
        icon,
        color,
        description,
        display_order: input.display_order,
        sector,
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
