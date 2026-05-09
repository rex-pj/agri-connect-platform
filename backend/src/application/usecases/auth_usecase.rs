use std::sync::Arc;

use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use rand::{rngs::OsRng, Rng};
use tracing::error;

use crate::{
    application::shared::{validate_password, validate_username},
    constants::{
        JWT_EXPIRY_HOURS, LOGIN_FAIL_MAX_ATTEMPTS, LOGIN_LOCK_MINUTES,
        PASSWORD_RESET_EXPIRY_HOURS, ROLE_ADMIN, ROLE_PROVIDER,
    },
    domain::{
        models::user::{NewResetToken, NewUser},
        repositories::{
            business_repository_trait::BusinessRepositoryTrait,
            user_repository_trait::UserRepositoryTrait,
        },
    },
    infrastructure::email_service::EmailService,
    middleware::auth::{Claims, Role},
    view_models::{
        responses::{LoginResponse, MeResponse},
        AppError,
    },
};

// ── Use case ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AuthUseCase {
    user_repo:     Arc<dyn UserRepositoryTrait     + Send + Sync>,
    business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
    email:         EmailService,
}

impl AuthUseCase {
    pub fn new(
        user_repo:     Arc<dyn UserRepositoryTrait     + Send + Sync>,
        business_repo: Arc<dyn BusinessRepositoryTrait + Send + Sync>,
        email:         EmailService,
    ) -> Self {
        Self { user_repo, business_repo, email }
    }

    // ── Login ─────────────────────────────────────────────────────────────────

    pub async fn login(&self, username: String, password: String) -> Result<LoginResponse, AppError> {
        let username = username.trim().to_lowercase();

        let credentials = self
            .user_repo
            .find_credentials_by_username(&username)
            .await?
            .ok_or_else(|| {
                AppError::Unauthorized("Tên đăng nhập hoặc mật khẩu không đúng.".into())
            })?;

        if let Some(locked_until) = credentials.locked_until {
            if locked_until > Utc::now() {
                return Err(AppError::AccountLocked(
                    "Tài khoản tạm thời bị khoá do đăng nhập sai quá nhiều lần. Vui lòng thử lại sau.".into(),
                ));
            }
        }

        let ok = verify(&password, &credentials.password_hash).map_err(|e| {
            error!("bcrypt verify error: {e}");
            AppError::Internal("Lỗi xác thực.".into())
        })?;

        if !ok {
            let _ = self.user_repo.record_failed_login(credentials.id, LOGIN_FAIL_MAX_ATTEMPTS, LOGIN_LOCK_MINUTES).await;
            return Err(AppError::Unauthorized(
                "Tên đăng nhập hoặc mật khẩu không đúng.".into(),
            ));
        }

        let _ = self.user_repo.reset_failed_login(credentials.id).await;

        let role = match credentials.role.as_str() {
            ROLE_ADMIN    => Role::Admin,
            ROLE_PROVIDER => Role::Provider,
            _ => return Err(AppError::Forbidden("Tài khoản không có quyền truy cập.".into())),
        };

        let token = mint_token(credentials.id, &username, role)?;
        Ok(LoginResponse { token, username, role: credentials.role })
    }

    // ── First-admin bootstrap ─────────────────────────────────────────────────

    pub async fn create_first_admin(
        &self,
        username:  String,
        email:     String,
        password:  String,
        full_name: Option<String>,
    ) -> Result<LoginResponse, AppError> {
        let admin_count = self.user_repo.count_by_role(ROLE_ADMIN).await?;
        if admin_count > 0 {
            return Err(AppError::Forbidden(
                "Hệ thống đã có tài khoản admin. Vui lòng đăng nhập.".into(),
            ));
        }

        let username = username.trim().to_lowercase();
        validate_username(&username)?;
        validate_password(&password)?;

        let password_hash = bcrypt_hash(&password)?;
        let email     = email.trim().to_lowercase();
        let full_name = full_name.as_deref().unwrap_or("Administrator").to_owned();

        let user_id = self
            .user_repo
            .create(NewUser {
                username:      username.clone(),
                email,
                password_hash,
                role:          ROLE_ADMIN.into(),
                full_name:     Some(full_name),
            })
            .await
            .map_err(|_| AppError::Internal("Không thể tạo tài khoản admin.".into()))?;

        let token = mint_token(user_id, &username, Role::Admin)?;
        Ok(LoginResponse { token, username, role: ROLE_ADMIN.into() })
    }

    // ── Provider self-registration ────────────────────────────────────────────

    pub async fn register(
        &self,
        username:       String,
        email:          String,
        password:       String,
        full_name:      Option<String>,
        business_name:  String,
        business_phone: String,
    ) -> Result<LoginResponse, AppError> {
        let username = username.trim().to_lowercase();
        validate_username(&username)?;
        validate_password(&password)?;

        let business_name  = business_name.trim().to_owned();
        let business_phone = business_phone.trim().to_owned();
        if business_name.is_empty() {
            return Err(AppError::BadRequest("Tên doanh nghiệp không được để trống.".into()));
        }
        if business_phone.is_empty() {
            return Err(AppError::BadRequest("Số điện thoại doanh nghiệp không được để trống.".into()));
        }

        let email = email.trim().to_lowercase();

        if self.user_repo.find_by_username(&username).await?.is_some() {
            return Err(AppError::Conflict("Tên đăng nhập đã tồn tại.".into()));
        }
        if self.user_repo.email_exists(&email).await? {
            return Err(AppError::Conflict("Email đã được sử dụng.".into()));
        }

        let password_hash = bcrypt_hash(&password)?;

        let user_id = self
            .user_repo
            .create(NewUser {
                username:      username.clone(),
                email,
                password_hash,
                role:          ROLE_PROVIDER.into(),
                full_name,
            })
            .await
            .map_err(|_| AppError::Internal("Không thể tạo tài khoản.".into()))?;

        self.business_repo
            .create_for_registration(&business_name, &business_phone, user_id)
            .await?;

        let token = mint_token(user_id, &username, Role::Provider)?;
        Ok(LoginResponse { token, username, role: ROLE_PROVIDER.into() })
    }

    // ── Me (authenticated) ────────────────────────────────────────────────────

    pub async fn me(&self, user_id: i32) -> Result<MeResponse, AppError> {
        let user = self
            .user_repo
            .find_me(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Tài khoản không tồn tại.".into()))?;

        Ok(MeResponse {
            username:   user.username,
            email:      user.email,
            full_name:  user.full_name,
            created_on: user.created_on.to_rfc3339(),
        })
    }

    // ── Change password (authenticated) ──────────────────────────────────────

    pub async fn change_password(
        &self,
        user_id:          i32,
        current_password: String,
        new_password:     String,
    ) -> Result<(), AppError> {
        validate_password(&new_password)?;

        let credentials = self
            .user_repo
            .find_credentials_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Tài khoản không tồn tại.".into()))?;

        let ok = verify(&current_password, &credentials.password_hash).map_err(|e| {
            error!("bcrypt verify error: {e}");
            AppError::Internal("Lỗi xác thực.".into())
        })?;

        if !ok {
            return Err(AppError::Unauthorized("Mật khẩu hiện tại không đúng.".into()));
        }

        let new_hash = bcrypt_hash(&new_password)?;
        self.user_repo.update_password(user_id, &new_hash).await
    }

    // ── Forgot password ───────────────────────────────────────────────────────

    pub async fn forgot_password(&self, email: String) -> Result<(), AppError> {
        let email = email.trim().to_lowercase();

        let user = match self.user_repo.find_by_email(&email).await? {
            None    => return Ok(()),
            Some(u) => u,
        };

        let token      = generate_token();
        let expires_at = Utc::now() + Duration::hours(PASSWORD_RESET_EXPIRY_HOURS);

        self.user_repo
            .create_reset_token(NewResetToken { user_id: user.id, token: token.clone(), expires_at })
            .await?;

        self.email.send_password_reset(&user.email, &token).await;

        Ok(())
    }

    // ── Reset password ────────────────────────────────────────────────────────

    pub async fn reset_password(
        &self,
        token:        String,
        new_password: String,
    ) -> Result<(), AppError> {
        validate_password(&new_password)?;

        let reset_token = self
            .user_repo
            .find_valid_reset_token(&token)
            .await?
            .ok_or_else(|| AppError::BadRequest(
                "Liên kết đặt lại mật khẩu không hợp lệ hoặc đã hết hạn.".into(),
            ))?;

        let new_hash = bcrypt_hash(&new_password)?;
        self.user_repo.update_password(reset_token.user_id, &new_hash).await?;
        self.user_repo.consume_reset_token(reset_token.id).await?;
        let _ = self.user_repo.reset_failed_login(reset_token.user_id).await;

        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn mint_token(user_id: i32, username: &str, role: Role) -> Result<String, AppError> {
    let exp    = (Utc::now() + Duration::hours(JWT_EXPIRY_HOURS)).timestamp() as usize;
    let claims = Claims { sub: user_id, role, username: username.to_owned(), exp };
    let secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET environment variable must be set");

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| {
        error!("JWT encode error: {e}");
        AppError::Internal("Không thể tạo phiên đăng nhập.".into())
    })
}

fn bcrypt_hash(password: &str) -> Result<String, AppError> {
    hash(password, DEFAULT_COST).map_err(|e| {
        error!("bcrypt hash error: {e}");
        AppError::Internal("Lỗi mã hoá mật khẩu.".into())
    })
}

fn generate_token() -> String {
    let bytes: [u8; 32] = OsRng.gen();
    hex::encode(bytes)
}
