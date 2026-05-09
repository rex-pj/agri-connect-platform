use async_trait::async_trait;
use chrono::Utc;
use sea_orm::sea_query::{Expr, SimpleExpr};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set,
};
use std::sync::Arc;
use tracing::error;

use crate::{
    domain::{
        models::user::{
            ExistingUser, NewResetToken, NewUser, ResetToken, UserAdmin, UserAdminDetail,
            UserByEmail, UserCredentials, UserMe,
        },
        repositories::user_repository_trait::UserRepositoryTrait,
    },
    entities::{
        password_reset_token::{self, Entity as PasswordResetToken},
        role::{self, Entity as Role},
        user::{self, Entity as UserEntity},
    },
    infrastructure::db_types::DateTimeWithTimeZone,
    view_models::AppError,
};

pub struct UserRepository {
    db: Arc<DatabaseConnection>,
}

impl UserRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[derive(FromQueryResult)]
struct CredentialsRow {
    id: i32,
    password_hash: String,
    role: String,
    locked_until: Option<DateTimeWithTimeZone>,
}

#[derive(FromQueryResult)]
struct UserMeRow {
    username: String,
    email: String,
    full_name: Option<String>,
    created_on: DateTimeWithTimeZone,
}

#[derive(FromQueryResult)]
struct UserByUsernameRow {
    id: i32,
    role: String,
}

#[derive(FromQueryResult)]
struct UserAdminRow {
    id: i32,
    username: String,
    email: String,
    full_name: Option<String>,
    role_code: String,
    role_name: String,
    is_active: bool,
}

#[derive(FromQueryResult)]
struct UserAdminDetailRow {
    id: i32,
    username: String,
    email: String,
    full_name: Option<String>,
    role_code: String,
    role_name: String,
    is_active: bool,
    created_on: DateTimeWithTimeZone,
}

#[async_trait]
impl UserRepositoryTrait for UserRepository {
    async fn find_credentials_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserCredentials>, AppError> {
        UserEntity::find()
            .select_only()
            .column(user::Column::Id)
            .column(user::Column::PasswordHash)
            .column(user::Column::LockedUntil)
            .column_as(role::Column::Code, "role")
            .join(JoinType::InnerJoin, user::Relation::Role.def())
            .filter(user::Column::Username.eq(username))
            .filter(user::Column::IsActive.eq(true))
            .filter(user::Column::DeletedAt.is_null())
            .into_model::<CredentialsRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| UserCredentials {
                    id: r.id,
                    password_hash: r.password_hash,
                    role: r.role,
                    locked_until: r.locked_until.map(|t| t.with_timezone(&Utc)),
                })
            })
            .map_err(|e| {
                error!("user find_credentials_by_username error: {e}");
                AppError::Internal("System error.".into())
            })
    }

    async fn count_by_role(&self, role: &str) -> Result<i64, AppError> {
        UserEntity::find()
            .join(JoinType::InnerJoin, user::Relation::Role.def())
            .filter(role::Column::Code.eq(role))
            .filter(user::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("user count_by_role error: {e}");
                AppError::Internal("System error.".into())
            })
    }

    async fn create(&self, user: NewUser) -> Result<i32, AppError> {
        let db = self.db.as_ref();

        let role_model = Role::find()
            .filter(role::Column::Code.eq(&user.role))
            .one(db)
            .await
            .map_err(|e| {
                error!("user create role lookup error: {e}");
                AppError::Internal("Failed to create account.".into())
            })?
            .ok_or_else(|| AppError::NotFound("Role not found.".into()))?;

        let new_user = user::ActiveModel {
            username: Set(user.username),
            email: Set(user.email),
            password_hash: Set(user.password_hash),
            role_id: Set(role_model.id),
            full_name: Set(user.full_name),
            is_active: Set(true),
            ..Default::default()
        };

        UserEntity::insert(new_user)
            .exec(db)
            .await
            .map(|r| r.last_insert_id)
            .map_err(|e| {
                error!("user create error: {e}");
                AppError::Internal("Failed to create account.".into())
            })
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<ExistingUser>, AppError> {
        UserEntity::find()
            .select_only()
            .column(user::Column::Id)
            .column_as(role::Column::Code, "role")
            .join(JoinType::InnerJoin, user::Relation::Role.def())
            .filter(user::Column::Username.eq(username))
            .filter(user::Column::DeletedAt.is_null())
            .into_model::<UserByUsernameRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| ExistingUser {
                    id: r.id,
                    role: r.role,
                })
            })
            .map_err(|e| {
                error!("user find_by_username error: {e}");
                AppError::Internal("Database error.".into())
            })
    }

    async fn count_all(&self) -> Result<i64, AppError> {
        UserEntity::find()
            .filter(user::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("user count_all error: {e}");
                AppError::Internal("Failed to load stats.".into())
            })
    }

    async fn list_for_admin(&self, limit: i64, offset: i64) -> Result<Vec<UserAdmin>, AppError> {
        UserEntity::find()
            .select_only()
            .column(user::Column::Id)
            .column(user::Column::Username)
            .column(user::Column::Email)
            .column(user::Column::FullName)
            .column_as(role::Column::Code, "role_code")
            .column_as(role::Column::Name, "role_name")
            .column(user::Column::IsActive)
            .join(JoinType::InnerJoin, user::Relation::Role.def())
            .filter(user::Column::DeletedAt.is_null())
            .order_by_asc(user::Column::Id)
            .limit(limit as u64)
            .offset(offset as u64)
            .into_model::<UserAdminRow>()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| UserAdmin {
                        id: r.id,
                        username: r.username,
                        email: r.email,
                        full_name: r.full_name,
                        role_code: r.role_code,
                        role_name: r.role_name,
                        is_active: r.is_active,
                    })
                    .collect()
            })
            .map_err(|e| {
                error!("user list_for_admin error: {e}");
                AppError::Internal("Failed to load users.".into())
            })
    }

    async fn find_by_id_for_admin(&self, id: i32) -> Result<Option<UserAdminDetail>, AppError> {
        UserEntity::find()
            .select_only()
            .column(user::Column::Id)
            .column(user::Column::Username)
            .column(user::Column::Email)
            .column(user::Column::FullName)
            .column_as(role::Column::Code, "role_code")
            .column_as(role::Column::Name, "role_name")
            .column(user::Column::IsActive)
            .column(user::Column::CreatedOn)
            .join(JoinType::InnerJoin, user::Relation::Role.def())
            .filter(user::Column::Id.eq(id))
            .filter(user::Column::DeletedAt.is_null())
            .into_model::<UserAdminDetailRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| UserAdminDetail {
                    id: r.id,
                    username: r.username,
                    email: r.email,
                    full_name: r.full_name,
                    role_code: r.role_code,
                    role_name: r.role_name,
                    is_active: r.is_active,
                    created_on: r.created_on.with_timezone(&Utc),
                })
            })
            .map_err(|e| {
                error!("user find_by_id_for_admin error: {e}");
                AppError::Internal("Failed to load user details.".into())
            })
    }

    async fn update_role(&self, user_id: i32, role_code: &str) -> Result<(), AppError> {
        let db = self.db.as_ref();

        let role_model = Role::find()
            .filter(role::Column::Code.eq(role_code))
            .one(db)
            .await
            .map_err(|e| {
                error!("user update_role role lookup error: {e}");
                AppError::Internal("Failed to update role.".into())
            })?
            .ok_or_else(|| AppError::NotFound("Role not found.".into()))?;

        let result = UserEntity::update_many()
            .col_expr(user::Column::RoleId, Expr::value(role_model.id))
            .filter(user::Column::Id.eq(user_id))
            .filter(user::Column::DeletedAt.is_null())
            .exec(db)
            .await
            .map_err(|e| {
                error!("user update_role error: {e}");
                AppError::Internal("Failed to update role.".into())
            })?;

        if result.rows_affected == 0 {
            return Err(AppError::NotFound("User not found.".into()));
        }
        Ok(())
    }

    async fn toggle_active(&self, user_id: i32, is_active: bool) -> Result<(), AppError> {
        let result = UserEntity::update_many()
            .col_expr(user::Column::IsActive, Expr::value(is_active))
            .filter(user::Column::Id.eq(user_id))
            .filter(user::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("user toggle_active error: {e}");
                AppError::Internal("Failed to update status.".into())
            })?;

        if result.rows_affected == 0 {
            return Err(AppError::NotFound("User not found.".into()));
        }
        Ok(())
    }

    async fn find_me(&self, user_id: i32) -> Result<Option<UserMe>, AppError> {
        UserEntity::find()
            .select_only()
            .column(user::Column::Username)
            .column(user::Column::Email)
            .column(user::Column::FullName)
            .column(user::Column::CreatedOn)
            .filter(user::Column::Id.eq(user_id))
            .filter(user::Column::IsActive.eq(true))
            .filter(user::Column::DeletedAt.is_null())
            .into_model::<UserMeRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| UserMe {
                    username: r.username,
                    email: r.email,
                    full_name: r.full_name,
                    created_on: r.created_on.with_timezone(&Utc),
                })
            })
            .map_err(|e| {
                error!("user find_me error: {e}");
                AppError::Internal("System error.".into())
            })
    }

    async fn find_credentials_by_id(&self, id: i32) -> Result<Option<UserCredentials>, AppError> {
        UserEntity::find()
            .select_only()
            .column(user::Column::Id)
            .column(user::Column::PasswordHash)
            .column(user::Column::LockedUntil)
            .column_as(role::Column::Code, "role")
            .join(JoinType::InnerJoin, user::Relation::Role.def())
            .filter(user::Column::Id.eq(id))
            .filter(user::Column::IsActive.eq(true))
            .filter(user::Column::DeletedAt.is_null())
            .into_model::<CredentialsRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| UserCredentials {
                    id: r.id,
                    password_hash: r.password_hash,
                    role: r.role,
                    locked_until: r.locked_until.map(|t| t.with_timezone(&Utc)),
                })
            })
            .map_err(|e| {
                error!("user find_credentials_by_id error: {e}");
                AppError::Internal("System error.".into())
            })
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<UserByEmail>, AppError> {
        UserEntity::find()
            .filter(user::Column::Email.eq(email))
            .filter(user::Column::IsActive.eq(true))
            .filter(user::Column::DeletedAt.is_null())
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| UserByEmail {
                    id: r.id,
                    email: r.email,
                })
            })
            .map_err(|e| {
                error!("user find_by_email error: {e}");
                AppError::Internal("System error.".into())
            })
    }

    async fn email_exists(&self, email: &str) -> Result<bool, AppError> {
        UserEntity::find()
            .filter(user::Column::Email.eq(email))
            .filter(user::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c > 0)
            .map_err(|e| {
                error!("user email_exists error: {e}");
                AppError::Internal("System error.".into())
            })
    }

    async fn update_password(&self, user_id: i32, password_hash: &str) -> Result<(), AppError> {
        let result = UserEntity::update_many()
            .col_expr(
                user::Column::PasswordHash,
                Expr::value(password_hash.to_string()),
            )
            .filter(user::Column::Id.eq(user_id))
            .filter(user::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("user update_password error: {e}");
                AppError::Internal("Failed to update password.".into())
            })?;

        if result.rows_affected == 0 {
            return Err(AppError::NotFound("User not found.".into()));
        }
        Ok(())
    }

    async fn create_reset_token(&self, t: NewResetToken) -> Result<(), AppError> {
        let new_token = password_reset_token::ActiveModel {
            user_id: Set(t.user_id),
            token: Set(t.token),
            expires_at: Set(t.expires_at.fixed_offset()),
            ..Default::default()
        };

        PasswordResetToken::insert(new_token)
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("create_reset_token error: {e}");
                AppError::Internal("Failed to create reset token.".into())
            })
    }

    async fn find_valid_reset_token(&self, token: &str) -> Result<Option<ResetToken>, AppError> {
        PasswordResetToken::find()
            .filter(password_reset_token::Column::Token.eq(token))
            .filter(password_reset_token::Column::UsedAt.is_null())
            .filter(password_reset_token::Column::ExpiresAt.gt(Utc::now().fixed_offset()))
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| ResetToken {
                    id: r.id,
                    user_id: r.user_id,
                })
            })
            .map_err(|e| {
                error!("find_valid_reset_token error: {e}");
                AppError::Internal("System error.".into())
            })
    }

    async fn consume_reset_token(&self, token_id: i32) -> Result<(), AppError> {
        PasswordResetToken::update_many()
            .col_expr(
                password_reset_token::Column::UsedAt,
                Expr::current_timestamp().into(),
            )
            .filter(password_reset_token::Column::Id.eq(token_id))
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("consume_reset_token error: {e}");
                AppError::Internal("Failed to consume token.".into())
            })
    }

    async fn record_failed_login(
        &self,
        user_id: i32,
        max_attempts: i32,
        lockout_minutes: i64,
    ) -> Result<(), AppError> {
        use sea_orm::ConnectionTrait;
        use sea_orm::DbBackend;
        use sea_orm::Statement;

        // Atomically increment and, if threshold is reached, set locked_until.
        let sql = r#"
            UPDATE users
            SET
                failed_login_attempts = failed_login_attempts + 1,
                locked_until = CASE
                    WHEN failed_login_attempts + 1 >= $1
                    THEN NOW() + ($2 * INTERVAL '1 minute')
                    ELSE locked_until
                END
            WHERE id = $3
        "#;

        self.db
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                [max_attempts.into(), lockout_minutes.into(), user_id.into()],
            ))
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("record_failed_login error: {e}");
                AppError::Internal("System error.".into())
            })
    }

    async fn reset_failed_login(&self, user_id: i32) -> Result<(), AppError> {
        let update: SimpleExpr = Expr::value(0);
        UserEntity::update_many()
            .col_expr(user::Column::FailedLoginAttempts, update)
            .col_expr(
                user::Column::LockedUntil,
                Expr::value(None::<DateTimeWithTimeZone>),
            )
            .filter(user::Column::Id.eq(user_id))
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("reset_failed_login error: {e}");
                AppError::Internal("System error.".into())
            })
    }
}
