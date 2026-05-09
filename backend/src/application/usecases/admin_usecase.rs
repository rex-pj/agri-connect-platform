use std::sync::Arc;

use bcrypt::{hash, DEFAULT_COST};
use tracing::error;

use crate::{
    application::shared::{validate_password, validate_username},
    constants::{ADMIN_PAGE_SIZE, CATEGORY_TYPES, INQUIRY_STATUSES, ROLE_PROVIDER},
    domain::{
        models::{
            category::{NewCategory, UpdateCategory},
            user::NewUser,
        },
        repositories::{
            business_repository_trait::BusinessRepositoryTrait,
            category_repository_trait::CategoryRepositoryTrait,
            inquiry_repository_trait::InquiryRepositoryTrait,
            review_repository_trait::ReviewRepositoryTrait,
            user_repository_trait::UserRepositoryTrait,
        },
    },
    helpers::{paginate, total_pages},
    view_models::{
        responses::{
            AdminCategoryDto, AdminInquiryDto, AdminPagedResponse, AdminProductItemDto,
            AdminProviderDetailDto, AdminProviderDto, AdminReviewDto, AdminServiceItemDto,
            AdminUserDetailDto, AdminUserDto, AssignUserDto, VerifyToggleDto,
        },
        AppError,
    },
};

// ── Use case ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AdminUseCase {
    business_repo:  Arc<dyn BusinessRepositoryTrait  + Send + Sync>,
    user_repo:      Arc<dyn UserRepositoryTrait       + Send + Sync>,
    category_repo:  Arc<dyn CategoryRepositoryTrait   + Send + Sync>,
    inquiry_repo:   Arc<dyn InquiryRepositoryTrait    + Send + Sync>,
    review_repo:    Arc<dyn ReviewRepositoryTrait      + Send + Sync>,
}

impl AdminUseCase {
    pub fn new(
        business_repo: Arc<dyn BusinessRepositoryTrait  + Send + Sync>,
        user_repo:     Arc<dyn UserRepositoryTrait       + Send + Sync>,
        category_repo: Arc<dyn CategoryRepositoryTrait   + Send + Sync>,
        inquiry_repo:  Arc<dyn InquiryRepositoryTrait    + Send + Sync>,
        review_repo:   Arc<dyn ReviewRepositoryTrait      + Send + Sync>,
    ) -> Self {
        Self { business_repo, user_repo, category_repo, inquiry_repo, review_repo }
    }

    // ── Providers ─────────────────────────────────────────────────────────────

    pub async fn get_provider_detail(&self, id: i32) -> Result<AdminProviderDetailDto, AppError> {
        let b = self
            .business_repo
            .find_admin_detail(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy nhà cung cấp.".into()))?;

        Ok(AdminProviderDetailDto {
            id: b.id, name: b.name, phone: b.phone, email: b.email,
            description: b.description, address: b.address,
            is_verified: b.is_verified, is_active: b.is_active,
            created_on: b.created_on, linked_username: b.linked_username,
            services: b.services.into_iter().map(|s| AdminServiceItemDto {
                id: s.id, name: s.name, category: s.category,
                price_note: s.price_note, is_active: s.is_active,
            }).collect(),
            products: b.products.into_iter().map(|p| AdminProductItemDto {
                id: p.id, name: p.name, price: p.price,
                unit: p.unit, category: p.category, is_active: p.is_active,
            }).collect(),
        })
    }

    pub async fn list_providers(&self, page: i64) -> Result<AdminPagedResponse<AdminProviderDto>, AppError> {
        let (page, offset) = paginate(page, ADMIN_PAGE_SIZE);
        let total          = self.business_repo.count_all().await?;
        let items          = self.business_repo.list_for_admin(ADMIN_PAGE_SIZE, offset).await?;

        Ok(AdminPagedResponse {
            items: items.into_iter().map(|b| AdminProviderDto {
                id: b.id, name: b.name, phone: b.phone, address: b.address,
                is_verified: b.is_verified, is_active: b.is_active,
                service_count: b.service_count, product_count: b.product_count,
                created_on: b.created_on, linked_username: b.linked_username,
            }).collect(),
            total, page, total_pages: total_pages(total, ADMIN_PAGE_SIZE),
        })
    }

    pub async fn toggle_verify(&self, id: i32) -> Result<VerifyToggleDto, AppError> {
        let result = self
            .business_repo
            .toggle_verify(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy nhà cung cấp.".into()))?;

        let (id, name, is_verified) = result;
        let message = if is_verified {
            format!("Đã xác minh thành công nhà cung cấp «{name}».")
        } else {
            format!("Đã huỷ xác minh nhà cung cấp «{name}».")
        };

        Ok(VerifyToggleDto { id, name, is_verified, message })
    }

    pub async fn assign_provider_user(
        &self,
        business_id: i32,
        username: String,
        email: String,
        password: String,
    ) -> Result<AssignUserDto, AppError> {
        let username = username.trim().to_lowercase();
        validate_username(&username)?;
        validate_password(&password)?;

        let biz_exists = self.business_repo.check_exists_for_admin(business_id).await?;
        if !biz_exists {
            return Err(AppError::NotFound("Không tìm thấy doanh nghiệp.".into()));
        }

        let user_id = match self.user_repo.find_by_username(&username).await? {
            Some(u) if u.role != ROLE_PROVIDER => {
                return Err(AppError::Conflict(
                    "Tên đăng nhập đã tồn tại với vai trò khác (không phải provider).".into(),
                ))
            }
            Some(u) => u.id,
            None => {
                let password_hash = hash(&password, DEFAULT_COST).map_err(|e| {
                    error!("bcrypt hash error: {e}");
                    AppError::Internal("Lỗi mã hoá mật khẩu.".into())
                })?;
                let email = email.trim().to_lowercase();

                self.user_repo
                    .create(NewUser {
                        username:      username.clone(),
                        email,
                        password_hash,
                        role:          ROLE_PROVIDER.into(),
                        full_name:     None,
                    })
                    .await
                    .map_err(|_| AppError::Internal("Không thể tạo tài khoản provider.".into()))?
            }
        };

        self.business_repo.assign_user(business_id, user_id).await?;

        Ok(AssignUserDto {
            business_id,
            username: username.clone(),
            message: format!("Đã gán tài khoản «{username}» cho doanh nghiệp thành công."),
        })
    }

    // ── Users ─────────────────────────────────────────────────────────────────

    pub async fn get_user_detail(&self, id: i32) -> Result<AdminUserDetailDto, AppError> {
        let u = self
            .user_repo
            .find_by_id_for_admin(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy người dùng.".into()))?;

        Ok(AdminUserDetailDto {
            id: u.id, username: u.username, email: u.email, full_name: u.full_name,
            role_code: u.role_code, role_name: u.role_name,
            is_active: u.is_active, created_on: u.created_on,
        })
    }

    pub async fn list_users(&self, page: i64) -> Result<AdminPagedResponse<AdminUserDto>, AppError> {
        let (page, offset) = paginate(page, ADMIN_PAGE_SIZE);
        let total          = self.user_repo.count_all().await?;
        let items          = self.user_repo.list_for_admin(ADMIN_PAGE_SIZE, offset).await?;

        Ok(AdminPagedResponse {
            items: items.into_iter().map(|u| AdminUserDto {
                id: u.id, username: u.username, email: u.email,
                full_name: u.full_name, role_code: u.role_code,
                role_name: u.role_name, is_active: u.is_active,
            }).collect(),
            total, page, total_pages: total_pages(total, ADMIN_PAGE_SIZE),
        })
    }

    pub async fn update_user_role(&self, user_id: i32, role_code: String) -> Result<(), AppError> {
        if role_code.trim().is_empty() {
            return Err(AppError::BadRequest("Mã vai trò không được để trống.".into()));
        }
        self.user_repo.update_role(user_id, &role_code).await
    }

    pub async fn toggle_user_active(&self, user_id: i32, is_active: bool) -> Result<(), AppError> {
        self.user_repo.toggle_active(user_id, is_active).await
    }

    // ── Categories ────────────────────────────────────────────────────────────

    pub async fn get_category_detail(&self, id: i32) -> Result<AdminCategoryDto, AppError> {
        let c = self
            .category_repo
            .find_by_id_admin(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Không tìm thấy danh mục.".into()))?;

        Ok(AdminCategoryDto {
            id: c.id, name: c.name, slug: c.slug, category_type: c.category_type,
            icon: c.icon, display_order: c.display_order, is_active: c.is_active,
        })
    }

    pub async fn list_categories(&self) -> Result<Vec<AdminCategoryDto>, AppError> {
        self.category_repo.list_admin().await.map(|cats| {
            cats.into_iter()
                .map(|c| AdminCategoryDto {
                    id: c.id, name: c.name, slug: c.slug,
                    category_type: c.category_type, icon: c.icon,
                    display_order: c.display_order, is_active: c.is_active,
                })
                .collect()
        })
    }

    pub async fn create_category(
        &self,
        name: String,
        slug: String,
        category_type: String,
        icon: Option<String>,
        display_order: i16,
    ) -> Result<i32, AppError> {
        let name = name.trim().to_string();
        let slug = slug.trim().to_string();
        if name.is_empty() || slug.is_empty() {
            return Err(AppError::BadRequest("Tên và slug không được để trống.".into()));
        }
        if !CATEGORY_TYPES.contains(&category_type.as_str()) {
            return Err(AppError::BadRequest(
                format!("Loại danh mục phải là: {}.", CATEGORY_TYPES.join(", ")),
            ));
        }
        self.category_repo.create(NewCategory { name, slug, category_type, icon, display_order }).await
    }

    pub async fn update_category(
        &self,
        id: i32,
        name: String,
        slug: String,
        icon: Option<String>,
        display_order: i16,
    ) -> Result<(), AppError> {
        let name = name.trim().to_string();
        let slug = slug.trim().to_string();
        if name.is_empty() || slug.is_empty() {
            return Err(AppError::BadRequest("Tên và slug không được để trống.".into()));
        }
        self.category_repo.update(id, UpdateCategory { name, slug, icon, display_order }).await
    }

    pub async fn toggle_category_active(&self, id: i32, is_active: bool) -> Result<(), AppError> {
        self.category_repo.toggle_active(id, is_active).await
    }

    // ── Inquiries ─────────────────────────────────────────────────────────────

    pub async fn list_inquiries(&self, page: i64) -> Result<AdminPagedResponse<AdminInquiryDto>, AppError> {
        let (page, offset) = paginate(page, ADMIN_PAGE_SIZE);
        let total          = self.inquiry_repo.count_admin().await?;
        let items          = self.inquiry_repo.list_admin(ADMIN_PAGE_SIZE, offset).await?;

        Ok(AdminPagedResponse {
            items: items.into_iter().map(|i| AdminInquiryDto {
                id: i.id, business_id: i.business_id,
                business_name: i.business_name, requester_name: i.requester_name,
                requester_phone: i.requester_phone, topic: i.topic,
                status: i.status, created_on: i.created_on,
            }).collect(),
            total, page, total_pages: total_pages(total, ADMIN_PAGE_SIZE),
        })
    }

    pub async fn update_inquiry_status(&self, id: i32, status: String) -> Result<(), AppError> {
        if !INQUIRY_STATUSES.contains(&status.as_str()) {
            return Err(AppError::BadRequest(
                format!("Trạng thái không hợp lệ. Phải là: {}.", INQUIRY_STATUSES.join(", ")),
            ));
        }
        self.inquiry_repo.update_status(id, &status).await
    }

    // ── Reviews ───────────────────────────────────────────────────────────────

    pub async fn list_reviews(&self, page: i64) -> Result<AdminPagedResponse<AdminReviewDto>, AppError> {
        let (page, offset) = paginate(page, ADMIN_PAGE_SIZE);
        let total          = self.review_repo.count_admin().await?;
        let items          = self.review_repo.list_admin(ADMIN_PAGE_SIZE, offset).await?;

        Ok(AdminPagedResponse {
            items: items.into_iter().map(|r| AdminReviewDto {
                id: r.id, business_id: r.business_id,
                business_name: r.business_name, reviewer_name: r.reviewer_name,
                rating: r.rating, comment: r.comment,
                is_visible: r.is_visible, created_on: r.created_on,
            }).collect(),
            total, page, total_pages: total_pages(total, ADMIN_PAGE_SIZE),
        })
    }

    pub async fn toggle_review_visible(&self, id: i32, is_visible: bool) -> Result<(), AppError> {
        self.review_repo.toggle_visible(id, is_visible).await
    }
}
