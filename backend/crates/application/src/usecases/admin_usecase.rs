use std::sync::Arc;

use tracing::warn;

use domain::{
    constants::{CATEGORY_TYPES, INQUIRY_STATUSES, ROLE_PROVIDER},
    models::{
        category::{CategoryAdmin, NewCategory, UpdateCategory},
        location::Location,
        role::{RoleModel, UserRole},
        service::{AdminService, NewService, UpdateService},
        user::NewUser,
    },
    repositories::{
        audit_log_repository_trait::AuditLogRepositoryTrait,
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
    constants::ADMIN_PAGE_SIZE,
    inputs::CategoryInput,
    outputs::{
        AdminCategoryDto, AdminInquiryDto, AdminLocationDto, AdminPagedResponse, AdminProductDto,
        AdminProductItemDto, AdminProviderDetailDto, AdminProviderDto, AdminReviewDto,
        AdminServiceDto, AdminServiceItemDto, AdminUserDetailDto, AdminUserDto, AssignUserDto,
        RoleDto, VerifyToggleDto,
    },
    pagination::{paginate, total_pages},
    ports::PasswordHasher,
    validation::{non_blank, validate_coordinates, validate_password, validate_username},
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
    audit_log_repo: Arc<dyn AuditLogRepositoryTrait + Send + Sync>,
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
    pub audit_log_repo: Arc<dyn AuditLogRepositoryTrait + Send + Sync>,
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
            audit_log_repo: d.audit_log_repo,
            password_hasher: d.password_hasher,
        }
    }

    /// Write an audit log entry. Failures are non-fatal — logs a warning,
    /// never rolls back the primary operation.
    async fn audit(
        &self,
        admin_id: i32,
        action: &str,
        entity_type: &str,
        entity_id: i32,
        reason: Option<String>,
    ) {
        if let Err(e) = self
            .audit_log_repo
            .log(admin_id, action, entity_type, entity_id, reason)
            .await
        {
            warn!(
                "audit log write failed (action={action}, entity={entity_type}/{entity_id}): {e:?}"
            );
        }
    }

    // ── Providers ─────────────────────────────────────────────────────────────

    pub async fn get_provider_detail(&self, id: i32) -> Result<AdminProviderDetailDto> {
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

    pub async fn list_providers(&self, page: i64) -> Result<AdminPagedResponse<AdminProviderDto>> {
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
        id: i32,
        admin_id: i32,
        reason: Option<String>,
    ) -> Result<VerifyToggleDto> {
        let result = self
            .business_repo
            .toggle_verify(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ProviderNotFound))?;

        let (id, name, is_verified) = result;
        let action = if is_verified {
            "verify_provider"
        } else {
            "unverify_provider"
        };
        self.audit(admin_id, action, "provider", id, reason).await;
        Ok(VerifyToggleDto {
            id,
            name,
            is_verified,
        })
    }

    pub async fn assign_provider_user(
        &self,
        business_id: i32,
        username: String,
        email: String,
        password: String,
    ) -> Result<AssignUserDto> {
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

    pub async fn get_user_detail(&self, id: i32) -> Result<AdminUserDetailDto> {
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

    pub async fn list_users(&self, page: i64) -> Result<AdminPagedResponse<AdminUserDto>> {
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
        user_id: i32,
        role_code: String,
    ) -> Result<AdminUserDetailDto> {
        // Both checks matter: the roles table says the role is active, the enum
        // says the code knows how to authorize it. Arbitrary strings must never
        // reach the role column.
        let code = role_code.trim();
        if self.role_repo.find_active_by_code(code).await?.is_none() {
            return Err(Error::new(ErrorCode::InvalidRole));
        }
        let role = UserRole::from_code(code).ok_or_else(|| Error::new(ErrorCode::InvalidRole))?;
        self.user_repo.update_role(user_id, &role).await?;
        self.get_user_detail(user_id).await
    }

    pub async fn toggle_user_active(
        &self,
        user_id: i32,
        is_active: bool,
        admin_id: i32,
        reason: Option<String>,
    ) -> Result<AdminUserDetailDto> {
        self.user_repo.toggle_active(user_id, is_active).await?;
        let action = if is_active {
            "activate_user"
        } else {
            "deactivate_user"
        };
        self.audit(admin_id, action, "user", user_id, reason).await;
        self.get_user_detail(user_id).await
    }

    // ── Categories ────────────────────────────────────────────────────────────

    pub async fn get_category_detail(&self, id: i32) -> Result<AdminCategoryDto> {
        self.category_repo
            .find_by_id_admin(id)
            .await?
            .map(to_category_dto)
            .ok_or_else(|| Error::new(ErrorCode::CategoryNotFound))
    }

    pub async fn list_categories(&self) -> Result<Vec<AdminCategoryDto>> {
        let cats = self.category_repo.list_admin().await?;
        Ok(cats.into_iter().map(to_category_dto).collect())
    }

    pub async fn create_category(
        &self,
        category_type: String,
        input: CategoryInput,
        admin_id: i32,
    ) -> Result<AdminCategoryDto> {
        let input = validate_category(input)?;
        if !CATEGORY_TYPES.contains(&category_type.as_str()) {
            return Err(Error::new(ErrorCode::InvalidCategoryType));
        }
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
                created_by: admin_id,
            })
            .await?;
        self.get_category_detail(id).await
    }

    pub async fn update_category(
        &self,
        id: i32,
        input: CategoryInput,
        admin_id: i32,
    ) -> Result<AdminCategoryDto> {
        let input = validate_category(input)?;
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
                    updated_by: admin_id,
                },
            )
            .await?;
        self.get_category_detail(id).await
    }

    pub async fn toggle_category_active(
        &self,
        id: i32,
        is_active: bool,
        admin_id: i32,
        reason: Option<String>,
    ) -> Result<AdminCategoryDto> {
        self.category_repo
            .toggle_active(id, is_active, admin_id)
            .await?;
        let action = if is_active {
            "show_category"
        } else {
            "hide_category"
        };
        self.audit(admin_id, action, "category", id, reason).await;
        self.get_category_detail(id).await
    }

    // ── Inquiries ─────────────────────────────────────────────────────────────

    pub async fn list_inquiries(&self, page: i64) -> Result<AdminPagedResponse<AdminInquiryDto>> {
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
        id: i32,
        status: String,
        admin_id: i32,
    ) -> Result<AdminInquiryDto> {
        if !INQUIRY_STATUSES.contains(&status.as_str()) {
            return Err(Error::new(ErrorCode::InvalidInquiryStatus));
        }
        self.inquiry_repo
            .update_status(id, &status, admin_id)
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

    pub async fn list_reviews(&self, page: i64) -> Result<AdminPagedResponse<AdminReviewDto>> {
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
        id: i32,
        is_visible: bool,
        admin_id: i32,
        reason: Option<String>,
    ) -> Result<AdminReviewDto> {
        self.review_repo
            .toggle_visible(id, is_visible, admin_id)
            .await?;
        let action = if is_visible {
            "show_review"
        } else {
            "hide_review"
        };
        self.audit(admin_id, action, "review", id, reason).await;
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

    pub async fn list_locations_admin(&self) -> Result<Vec<AdminLocationDto>> {
        self.location_repo
            .list_all()
            .await
            .map(|locs| locs.into_iter().map(to_location_dto).collect())
    }

    pub async fn get_location_admin(&self, id: i32) -> Result<AdminLocationDto> {
        self.location_repo
            .find_by_id(id)
            .await?
            .map(to_location_dto)
            .ok_or_else(|| Error::new(ErrorCode::LocationNotFound))
    }

    pub async fn create_location(
        &self,
        label: String,
        lat: f64,
        lng: f64,
        display_order: i32,
    ) -> Result<AdminLocationDto> {
        let label = label.trim().to_string();
        if label.is_empty() {
            return Err(Error::new(ErrorCode::LabelRequired));
        }
        validate_coordinates(lat, lng)?;
        let id = self
            .location_repo
            .create(label, lat, lng, display_order)
            .await?;
        self.get_location_admin(id).await
    }

    pub async fn update_location(
        &self,
        id: i32,
        label: String,
        lat: f64,
        lng: f64,
        display_order: i32,
    ) -> Result<AdminLocationDto> {
        let label = label.trim().to_string();
        if label.is_empty() {
            return Err(Error::new(ErrorCode::LabelRequired));
        }
        validate_coordinates(lat, lng)?;
        self.location_repo
            .update(id, label, lat, lng, display_order)
            .await?;
        self.get_location_admin(id).await
    }

    pub async fn toggle_location_active(
        &self,
        id: i32,
        is_active: bool,
        admin_id: i32,
        reason: Option<String>,
    ) -> Result<AdminLocationDto> {
        self.location_repo.toggle_active(id, is_active).await?;
        let action = if is_active {
            "show_location"
        } else {
            "hide_location"
        };
        self.audit(admin_id, action, "location", id, reason).await;
        self.get_location_admin(id).await
    }

    pub async fn delete_location(
        &self,
        id: i32,
        admin_id: i32,
        reason: Option<String>,
    ) -> Result<()> {
        self.audit(admin_id, "delete_location", "location", id, reason)
            .await;
        self.location_repo.delete(id).await
    }

    // ── Admin Products ────────────────────────────────────────────────────────

    pub async fn list_products_admin(
        &self,
        page: i64,
        provider_id: Option<i32>,
    ) -> Result<AdminPagedResponse<AdminProductDto>> {
        let (page, offset) = paginate(page, ADMIN_PAGE_SIZE);
        let total = self.product_repo.count_admin(provider_id).await?;
        let items = self
            .product_repo
            .list_admin(provider_id, ADMIN_PAGE_SIZE, offset)
            .await?;
        Ok(AdminPagedResponse {
            items: items
                .into_iter()
                .map(|p| AdminProductDto {
                    id: p.id,
                    name: p.name,
                    description: p.description,
                    price: p.price,
                    unit: p.unit,
                    category: p.category,
                    category_id: p.category_id,
                    business_id: p.business_id,
                    business_name: p.business_name,
                    is_active: p.is_active,
                    created_on: p.created_on,
                    updated_on: p.updated_on,
                })
                .collect(),
            total,
            page,
            per_page: ADMIN_PAGE_SIZE,
            total_pages: total_pages(total, ADMIN_PAGE_SIZE),
        })
    }

    pub async fn get_product_admin(&self, id: i32) -> Result<AdminProductDto> {
        let p = self
            .product_repo
            .find_by_id_admin(id)
            .await?
            .ok_or_else(|| Error::new(ErrorCode::ProductNotFound))?;
        Ok(AdminProductDto {
            id: p.id,
            name: p.name,
            description: p.description,
            price: p.price,
            unit: p.unit,
            category: p.category,
            category_id: p.category_id,
            business_id: p.business_id,
            business_name: p.business_name,
            is_active: p.is_active,
            created_on: p.created_on,
            updated_on: p.updated_on,
        })
    }

    pub async fn toggle_product_active(
        &self,
        id: i32,
        is_active: bool,
        admin_id: i32,
        reason: Option<String>,
    ) -> Result<AdminProductDto> {
        let updated = self.product_repo.admin_toggle_active(id, is_active).await?;
        if !updated {
            return Err(Error::new(ErrorCode::ProductNotFound));
        }
        let action = if is_active {
            "show_product"
        } else {
            "hide_product"
        };
        self.audit(admin_id, action, "product", id, reason).await;
        self.get_product_admin(id).await
    }

    pub async fn delete_product_admin(
        &self,
        id: i32,
        admin_id: i32,
        reason: Option<String>,
    ) -> Result<()> {
        let deleted = self.product_repo.admin_delete(id).await?;
        if !deleted {
            return Err(Error::new(ErrorCode::ProductNotFound));
        }
        self.audit(admin_id, "delete_product", "product", id, reason)
            .await;
        Ok(())
    }

    // ── Admin Services ────────────────────────────────────────────────────────

    pub async fn list_services_admin(
        &self,
        page: i64,
        provider_id: Option<i32>,
    ) -> Result<AdminPagedResponse<AdminServiceDto>> {
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

    pub async fn get_service_admin(&self, id: i32) -> Result<AdminServiceDto> {
        self.service_repo
            .find_by_id_admin(id)
            .await?
            .map(to_service_dto)
            .ok_or_else(|| Error::new(ErrorCode::ServiceNotFound))
    }

    pub async fn create_service_admin(
        &self,
        business_id: i32,
        category_id: i32,
        name: String,
        description: Option<String>,
        price_note: Option<String>,
        admin_user_id: i32,
    ) -> Result<AdminServiceDto> {
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
                created_by: admin_user_id,
            })
            .await?;
        self.get_service_admin(id).await
    }

    pub async fn update_service_admin(
        &self,
        id: i32,
        category_id: i32,
        name: String,
        description: Option<String>,
        price_note: Option<String>,
        admin_user_id: i32,
    ) -> Result<AdminServiceDto> {
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
                updated_by: admin_user_id,
            })
            .await?;
        if !updated {
            return Err(Error::new(ErrorCode::ServiceNotFound));
        }
        self.get_service_admin(id).await
    }

    pub async fn toggle_service_active(
        &self,
        id: i32,
        is_active: bool,
        admin_id: i32,
        reason: Option<String>,
    ) -> Result<AdminServiceDto> {
        let updated = self.service_repo.toggle_active(id, is_active).await?;
        if !updated {
            return Err(Error::new(ErrorCode::ServiceNotFound));
        }
        let action = if is_active {
            "show_service"
        } else {
            "hide_service"
        };
        self.audit(admin_id, action, "service", id, reason).await;
        self.get_service_admin(id).await
    }

    pub async fn delete_service_admin(
        &self,
        id: i32,
        admin_id: i32,
        reason: Option<String>,
    ) -> Result<()> {
        let deleted = self.service_repo.delete(id).await?;
        if !deleted {
            return Err(Error::new(ErrorCode::ServiceNotFound));
        }
        self.audit(admin_id, "delete_service", "service", id, reason)
            .await;
        Ok(())
    }

    // ── Roles (read-only) ─────────────────────────────────────────────────────

    pub async fn list_roles(&self) -> Result<Vec<RoleDto>> {
        self.role_repo
            .list()
            .await
            .map(|roles| roles.into_iter().map(to_role_dto).collect())
    }

    pub async fn get_role_by_id(&self, id: i32) -> Result<RoleDto> {
        self.role_repo
            .find_by_id(id)
            .await?
            .map(to_role_dto)
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

fn to_role_dto(r: RoleModel) -> RoleDto {
    RoleDto {
        id: r.id,
        code: r.code,
        name: r.name,
        description: r.description,
        is_active: r.is_active,
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
fn validate_category(input: CategoryInput) -> Result<CategoryInput> {
    let name = input.name.trim().to_owned();
    let slug = input.slug.trim().to_owned();
    Validation::new()
        .check(name.is_empty(), "name", ErrorCode::NameRequired)
        .check(slug.is_empty(), "slug", ErrorCode::SlugRequired)
        .finish()?;
    Ok(CategoryInput {
        name,
        slug,
        icon: non_blank(input.icon),
        color: non_blank(input.color),
        description: non_blank(input.description),
        display_order: input.display_order,
    })
}
