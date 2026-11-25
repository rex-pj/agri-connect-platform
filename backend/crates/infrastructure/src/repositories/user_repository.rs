// std
use std::sync::Arc;

// external
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    sea_query::{Expr, Order, Query, SimpleExpr},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, FromQueryResult,
    JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set, SqlErr,
    TransactionTrait,
};
use tracing::error;

use domain::{
    models::{
        admin_list::AdminFilter,
        role::UserRole,
        user::{
            ClaimedEmail, ExistingUser, NewEmailVerification, NewResetToken, NewUser, UserAdmin,
            UserAdminDetail, UserByEmail, UserCredentials, UserMe,
        },
    },
    repositories::user_repository_trait::UserRepositoryTrait,
};

use domain::{Error, ErrorCode};

use crate::admin_list::{admin_order, switch_state, OrderColumns};
use crate::repositories::{business_repository::insert_registration_business, unique_or_internal};
use crate::search::{search_condition, ADMIN_USER_SEARCH_COLUMNS};
use crate::sql::{account_name_of, advisory_xact_lock, null, scalar, PgType, QueryAs};
use crate::{
    db_types::DateTimeWithTimeZone,
    entities::{
        email_verification_token::{self, Entity as EmailVerificationToken},
        password_reset_token::{self, Entity as PasswordResetToken},
        role::{self, Entity as Role},
        user::{self, Entity as UserEntity},
    },
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
    email: String,
    password_hash: String,
    role: String,
    /// Fetched explicitly so the application layer can enforce the
    /// "inactive accounts may not log in" invariant — not a silent SQL filter.
    is_active: bool,
    locked_until: Option<DateTimeWithTimeZone>,
}

#[derive(FromQueryResult)]
struct UserMeRow {
    email: String,
    full_name: Option<String>,
    role_code: String,
    created_on: DateTimeWithTimeZone,
    email_verified: bool,
    pending_email: Option<String>,
}

#[derive(FromQueryResult)]
struct AccountByEmailRow {
    id: i32,
    role: String,
    email_verified: bool,
}

#[derive(FromQueryResult)]
struct UserAdminRow {
    id: i32,
    email: String,
    full_name: Option<String>,
    role_code: String,
    role_name: String,
    is_active: bool,
    is_demo: bool,
    created_on: DateTimeWithTimeZone,
    updated_on: DateTimeWithTimeZone,
    created_by_name: Option<String>,
    updated_by_name: Option<String>,
}

#[derive(FromQueryResult)]
struct UserAdminDetailRow {
    id: i32,
    email: String,
    full_name: Option<String>,
    role_code: String,
    role_name: String,
    is_active: bool,
    created_on: DateTimeWithTimeZone,
    email_verified: bool,
    pending_email: Option<String>,
}

/// Key of the advisory lock that serialises first-admin creation.
const FIRST_ADMIN_LOCK: i64 = 0x0061_6772_695f_0001;

async fn count_by_role_in<C: ConnectionTrait>(db: &C, role: &str) -> Result<i64, Error> {
    UserEntity::find()
        .join(JoinType::InnerJoin, user::Relation::Role.def())
        .filter(role::Column::Code.eq(role))
        .filter(user::Column::DeletedAt.is_null())
        .count(db)
        .await
        .map(|c| c as i64)
        .map_err(|e| {
            error!("user count_by_role error: {e}");
            Error::internal("System error.")
        })
}

/// Inserts `user` with the id of its role, on a connection or inside a
/// transaction. A UNIQUE clash is a field error on the column it hit.
pub(crate) async fn insert_user<C: ConnectionTrait>(db: &C, user: NewUser) -> Result<i32, Error> {
    let role_model = Role::find()
        .filter(role::Column::Code.eq(&user.role))
        .one(db)
        .await
        .map_err(|e| {
            error!("user create role lookup error: {e}");
            Error::internal("Failed to create account.")
        })?
        .ok_or_else(|| Error::new(ErrorCode::RoleNotFound))?;

    let new_user = user::ActiveModel {
        email: Set(user.email),
        password_hash: Set(user.password_hash),
        role_id: Set(role_model.id),
        full_name: Set(user.full_name),
        email_verified_at: Set(user.email_verified.then(|| Utc::now().fixed_offset())),
        is_active: Set(true),
        ..Default::default()
    };

    UserEntity::insert(new_user)
        .exec(db)
        .await
        .map(|r| r.last_insert_id)
        // A concurrent sign-up took the address first.
        .map_err(|e| unique_or_internal(e, "email", ErrorCode::EmailTaken, "user create"))
}

impl UserRepository {
    /// The sign-in record of the one live (not deleted) account `condition`
    /// picks. `is_active` is NOT filtered: the use case enforces "inactive
    /// accounts may not log in" so the rule is visible at that layer.
    async fn credentials_where(
        &self,
        condition: SimpleExpr,
        what: &str,
    ) -> Result<Option<UserCredentials>, Error> {
        UserEntity::find()
            .select_only()
            .column(user::Column::Id)
            .column(user::Column::Email)
            .column(user::Column::PasswordHash)
            .column(user::Column::IsActive)
            .column(user::Column::LockedUntil)
            .column_as(role::Column::Code, QueryAs::Role)
            .join(JoinType::InnerJoin, user::Relation::Role.def())
            .filter(condition)
            .filter(user::Column::DeletedAt.is_null())
            .into_model::<CredentialsRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| UserCredentials {
                    id: r.id,
                    email: r.email,
                    password_hash: r.password_hash,
                    role: r.role,
                    is_active: r.is_active,
                    locked_until: r.locked_until.map(|t| t.with_timezone(&Utc)),
                })
            })
            .map_err(|e| {
                error!("user {what} error: {e}");
                Error::internal("System error.")
            })
    }
}

#[async_trait]
impl UserRepositoryTrait for UserRepository {
    async fn find_credentials(&self, email: &str) -> Result<Option<UserCredentials>, Error> {
        self.credentials_where(user::Column::Email.eq(email), "find_credentials")
            .await
    }

    async fn count_by_role(&self, role: &str) -> Result<i64, Error> {
        count_by_role_in(self.db.as_ref(), role).await
    }

    async fn create(&self, user: NewUser) -> Result<i32, Error> {
        insert_user(self.db.as_ref(), user).await
    }

    async fn create_first_admin(&self, user: NewUser) -> Result<Option<i32>, Error> {
        let fail = |e: DbErr| {
            error!("user create_first_admin error: {e}");
            Error::internal("Failed to create account.")
        };
        let txn = self.db.begin().await.map_err(fail)?;
        // Two setup forms sent at once must not both see "no admin yet": the
        // second waits here until the first commits, then sees its admin.
        txn.execute(
            txn.get_database_backend()
                .build(&advisory_xact_lock(FIRST_ADMIN_LOCK)),
        )
        .await
        .map_err(fail)?;
        if count_by_role_in(&txn, &user.role).await? > 0 {
            return Ok(None);
        }
        let id = insert_user(&txn, user).await?;
        txn.commit().await.map_err(fail)?;
        Ok(Some(id))
    }

    async fn create_with_business(
        &self,
        user: NewUser,
        business_name: &str,
        business_phone: &str,
    ) -> Result<i32, Error> {
        let fail = |e: DbErr| {
            error!("user create_with_business error: {e}");
            Error::internal("Failed to create account.")
        };
        // Dropped without commit on any error: the account goes too.
        let txn = self.db.begin().await.map_err(fail)?;
        let id = insert_user(&txn, user).await?;
        insert_registration_business(&txn, business_name, business_phone, id).await?;
        txn.commit().await.map_err(fail)?;
        Ok(id)
    }

    async fn find_account_by_email(&self, email: &str) -> Result<Option<ExistingUser>, Error> {
        UserEntity::find()
            .select_only()
            .column(user::Column::Id)
            .column_as(role::Column::Code, QueryAs::Role)
            .expr_as(email_verified(), QueryAs::EmailVerified)
            .join(JoinType::InnerJoin, user::Relation::Role.def())
            .filter(user::Column::Email.eq(email))
            .filter(user::Column::DeletedAt.is_null())
            .into_model::<AccountByEmailRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| ExistingUser {
                    id: r.id,
                    role: r.role,
                    email_verified: r.email_verified,
                })
            })
            .map_err(|e| {
                error!("user find_account_by_email error: {e}");
                Error::internal("Database error.")
            })
    }

    async fn count_all(&self) -> Result<i64, Error> {
        UserEntity::find()
            .filter(user::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("user count_all error: {e}");
                Error::internal("Failed to load stats.")
            })
    }

    async fn count_for_admin(&self, filter: &AdminFilter) -> Result<i64, Error> {
        UserEntity::find()
            .filter(user::Column::DeletedAt.is_null())
            .filter(admin_condition(filter))
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(|e| {
                error!("user count_for_admin error: {e}");
                Error::internal("Failed to count users.")
            })
    }

    async fn list_for_admin(
        &self,
        filter: &AdminFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<UserAdmin>, Error> {
        let query = UserEntity::find()
            .select_only()
            .column(user::Column::Id)
            .column(user::Column::Email)
            .column(user::Column::FullName)
            .column_as(role::Column::Code, QueryAs::RoleCode)
            .column_as(role::Column::Name, QueryAs::RoleName)
            .column(user::Column::IsActive)
            .column(user::Column::IsDemo)
            .column(user::Column::CreatedOn)
            .column(user::Column::UpdatedOn)
            .expr_as(
                account_name_of((user::Entity, user::Column::CreatedBy)),
                QueryAs::CreatedByName,
            )
            .expr_as(
                account_name_of((user::Entity, user::Column::UpdatedBy)),
                QueryAs::UpdatedByName,
            )
            .join(JoinType::LeftJoin, user::Relation::Role.def())
            .filter(user::Column::DeletedAt.is_null())
            .filter(admin_condition(filter));
        admin_order(
            query,
            filter.sort,
            OrderColumns {
                created: user::Column::CreatedOn,
                name: user::Column::Email,
                id: user::Column::Id,
            },
            |q| q.order_by_asc(user::Column::Id),
        )
        .limit(limit as u64)
        .offset(offset as u64)
        .into_model::<UserAdminRow>()
        .all(self.db.as_ref())
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|r| UserAdmin {
                    id: r.id,
                    email: r.email,
                    full_name: r.full_name,
                    role_code: r.role_code,
                    role_name: r.role_name,
                    is_active: r.is_active,
                    is_demo: r.is_demo,
                    created_on: r.created_on.with_timezone(&Utc),
                    updated_on: r.updated_on.with_timezone(&Utc),
                    created_by_name: r.created_by_name,
                    updated_by_name: r.updated_by_name,
                })
                .collect()
        })
        .map_err(|e| {
            error!("user list_for_admin error: {e}");
            Error::internal("Failed to load users.")
        })
    }

    async fn find_by_id_for_admin(&self, id: i32) -> Result<Option<UserAdminDetail>, Error> {
        UserEntity::find()
            .select_only()
            .column(user::Column::Id)
            .column(user::Column::Email)
            .column(user::Column::FullName)
            .column_as(role::Column::Code, QueryAs::RoleCode)
            .column_as(role::Column::Name, QueryAs::RoleName)
            .column(user::Column::IsActive)
            .column(user::Column::CreatedOn)
            .expr_as(email_verified(), QueryAs::EmailVerified)
            .expr_as(pending_email(), QueryAs::PendingEmail)
            .join(JoinType::InnerJoin, user::Relation::Role.def())
            .filter(user::Column::Id.eq(id))
            .filter(user::Column::DeletedAt.is_null())
            .into_model::<UserAdminDetailRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| UserAdminDetail {
                    id: r.id,
                    email: r.email,
                    full_name: r.full_name,
                    role_code: r.role_code,
                    role_name: r.role_name,
                    is_active: r.is_active,
                    created_on: r.created_on.with_timezone(&Utc),
                    email_verified: r.email_verified,
                    pending_email: r.pending_email,
                })
            })
            .map_err(|e| {
                error!("user find_by_id_for_admin error: {e}");
                Error::internal("Failed to load user details.")
            })
    }

    async fn update_role(&self, user_id: i32, role: &UserRole) -> Result<(), Error> {
        let db = self.db.as_ref();

        let role_model = Role::find()
            .filter(role::Column::Code.eq(role.as_code()))
            .one(db)
            .await
            .map_err(|e| {
                error!("user update_role role lookup error: {e}");
                Error::internal("Failed to update role.")
            })?
            .ok_or_else(|| Error::new(ErrorCode::RoleNotFound))?;

        let result = UserEntity::update_many()
            .col_expr(user::Column::RoleId, Expr::value(role_model.id))
            .filter(user::Column::Id.eq(user_id))
            .filter(user::Column::DeletedAt.is_null())
            .exec(db)
            .await
            .map_err(|e| {
                error!("user update_role error: {e}");
                Error::internal("Failed to update role.")
            })?;

        if result.rows_affected == 0 {
            return Err(Error::new(ErrorCode::UserNotFound));
        }
        Ok(())
    }

    async fn toggle_active(&self, user_id: i32, is_active: bool) -> Result<(), Error> {
        let result = UserEntity::update_many()
            .col_expr(user::Column::IsActive, Expr::value(is_active))
            .filter(user::Column::Id.eq(user_id))
            .filter(user::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("user toggle_active error: {e}");
                Error::internal("Failed to update status.")
            })?;

        if result.rows_affected == 0 {
            return Err(Error::new(ErrorCode::UserNotFound));
        }
        Ok(())
    }

    async fn find_me(&self, user_id: i32) -> Result<Option<UserMe>, Error> {
        UserEntity::find()
            .select_only()
            .column(user::Column::Email)
            .column(user::Column::FullName)
            .column(user::Column::CreatedOn)
            .column_as(role::Column::Code, QueryAs::RoleCode)
            .expr_as(email_verified(), QueryAs::EmailVerified)
            .expr_as(pending_email(), QueryAs::PendingEmail)
            .join(JoinType::InnerJoin, user::Relation::Role.def())
            .filter(user::Column::Id.eq(user_id))
            .filter(user::Column::IsActive.eq(true))
            .filter(user::Column::DeletedAt.is_null())
            .into_model::<UserMeRow>()
            .one(self.db.as_ref())
            .await
            .map(|opt| {
                opt.map(|r| UserMe {
                    email: r.email,
                    full_name: r.full_name,
                    role: r.role_code,
                    created_on: r.created_on.with_timezone(&Utc),
                    email_verified: r.email_verified,
                    pending_email: r.pending_email,
                })
            })
            .map_err(|e| {
                error!("user find_me error: {e}");
                Error::internal("System error.")
            })
    }

    async fn find_credentials_by_id(&self, id: i32) -> Result<Option<UserCredentials>, Error> {
        self.credentials_where(user::Column::Id.eq(id), "find_credentials_by_id")
            .await
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<UserByEmail>, Error> {
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
                Error::internal("System error.")
            })
    }

    async fn email_exists(&self, email: &str) -> Result<bool, Error> {
        UserEntity::find()
            .filter(user::Column::Email.eq(email))
            .filter(user::Column::DeletedAt.is_null())
            .count(self.db.as_ref())
            .await
            .map(|c| c > 0)
            .map_err(|e| {
                error!("user email_exists error: {e}");
                Error::internal("System error.")
            })
    }

    async fn end_sessions(&self, user_id: i32) -> Result<(), Error> {
        // This server's clock, not the database's: it is compared with the
        // `iat` this server writes into tokens, and a skew between the two
        // would refuse the fresh token of the browser that made the change.
        UserEntity::update_many()
            .col_expr(
                user::Column::SessionsValidAfter,
                Expr::value(Utc::now().fixed_offset()),
            )
            .filter(user::Column::Id.eq(user_id))
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("user end_sessions error: {e}");
                Error::internal("Failed to end sessions.")
            })
    }

    async fn update_password(&self, user_id: i32, password_hash: &str) -> Result<(), Error> {
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
                Error::internal("Failed to update password.")
            })?;

        if result.rows_affected == 0 {
            return Err(Error::new(ErrorCode::UserNotFound));
        }
        Ok(())
    }

    async fn update_full_name(&self, user_id: i32, full_name: Option<&str>) -> Result<(), Error> {
        let result = UserEntity::update_many()
            .col_expr(
                user::Column::FullName,
                Expr::value(full_name.map(str::to_owned)),
            )
            .filter(user::Column::Id.eq(user_id))
            .filter(user::Column::DeletedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("user update_full_name error: {e}");
                Error::internal("Failed to update the name.")
            })?;
        if result.rows_affected == 0 {
            return Err(Error::new(ErrorCode::UserNotFound));
        }
        Ok(())
    }

    async fn create_reset_token(&self, t: NewResetToken) -> Result<(), Error> {
        let new_token = password_reset_token::ActiveModel {
            user_id: Set(t.user_id),
            token: Set(t.token_hash),
            expires_at: Set(t.expires_at.fixed_offset()),
            ..Default::default()
        };

        PasswordResetToken::insert(new_token)
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("create_reset_token error: {e}");
                Error::internal("Failed to create reset token.")
            })
    }

    async fn count_live_reset_tokens(
        &self,
        user_id: i32,
        now: chrono::DateTime<Utc>,
    ) -> Result<u64, Error> {
        PasswordResetToken::find()
            .filter(password_reset_token::Column::UserId.eq(user_id))
            .filter(password_reset_token::Column::ExpiresAt.gt(now.fixed_offset()))
            .count(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("count_live_reset_tokens error: {e}");
                Error::internal("System error.")
            })
    }

    async fn reset_token_live(&self, token_hash: &str) -> Result<bool, Error> {
        PasswordResetToken::find()
            .filter(password_reset_token::Column::Token.eq(token_hash))
            .filter(password_reset_token::Column::UsedAt.is_null())
            .filter(
                Expr::col((PasswordResetToken, password_reset_token::Column::ExpiresAt))
                    .gt(Expr::current_timestamp()),
            )
            .count(self.db.as_ref())
            .await
            .map(|n| n > 0)
            .map_err(|e| {
                error!("reset_token_live error: {e}");
                Error::internal("System error.")
            })
    }

    async fn reset_password_with_token(
        &self,
        token_hash: &str,
        password_hash: &str,
    ) -> Result<Option<i32>, Error> {
        let fail = |e: DbErr| {
            error!("reset_password_with_token error: {e}");
            Error::internal("Failed to update password.")
        };
        // The link is spent and the password stored together, or neither: a
        // failure in between must not burn the link.
        let txn = self.db.begin().await.map_err(fail)?;
        // One UPDATE … RETURNING: two requests with the same link cannot
        // both claim it.
        let claimed = PasswordResetToken::update_many()
            .col_expr(
                password_reset_token::Column::UsedAt,
                Expr::current_timestamp().into(),
            )
            .filter(password_reset_token::Column::Token.eq(token_hash))
            .filter(password_reset_token::Column::UsedAt.is_null())
            .filter(
                Expr::col((PasswordResetToken, password_reset_token::Column::ExpiresAt))
                    .gt(Expr::current_timestamp()),
            )
            .exec_with_returning(&txn)
            .await
            .map_err(fail)?;
        let Some(token) = claimed.into_iter().next() else {
            return Ok(None);
        };
        let updated = UserEntity::update_many()
            .col_expr(
                user::Column::PasswordHash,
                Expr::value(password_hash.to_owned()),
            )
            .filter(user::Column::Id.eq(token.user_id))
            .filter(user::Column::DeletedAt.is_null())
            .exec(&txn)
            .await
            .map_err(fail)?;
        if updated.rows_affected == 0 {
            // The account is gone: nothing was reset, the link stays unspent.
            return Ok(None);
        }
        // Any other link still in the inbox stops working too.
        PasswordResetToken::update_many()
            .col_expr(
                password_reset_token::Column::UsedAt,
                Expr::current_timestamp().into(),
            )
            .filter(password_reset_token::Column::UserId.eq(token.user_id))
            .filter(password_reset_token::Column::UsedAt.is_null())
            .exec(&txn)
            .await
            .map_err(fail)?;
        txn.commit().await.map_err(fail)?;
        Ok(Some(token.user_id))
    }

    async fn delete_expired_links(&self, before: chrono::DateTime<Utc>) -> Result<u64, Error> {
        let fail = |e: DbErr| {
            error!("delete_expired_links error: {e}");
            Error::internal("System error.")
        };
        let resets = PasswordResetToken::delete_many()
            .filter(password_reset_token::Column::ExpiresAt.lt(before.fixed_offset()))
            .exec(self.db.as_ref())
            .await
            .map_err(fail)?;
        let confirmations = EmailVerificationToken::delete_many()
            .filter(email_verification_token::Column::ExpiresAt.lt(before.fixed_offset()))
            .exec(self.db.as_ref())
            .await
            .map_err(fail)?;
        Ok(resets.rows_affected + confirmations.rows_affected)
    }

    async fn create_email_verification(&self, t: NewEmailVerification) -> Result<(), Error> {
        EmailVerificationToken::insert(email_verification_token::ActiveModel {
            user_id: Set(t.user_id),
            token_hash: Set(t.token_hash),
            expires_at: Set(t.expires_at.fixed_offset()),
            new_email: Set(t.new_email),
            ..Default::default()
        })
        .exec(self.db.as_ref())
        .await
        .map(|_| ())
        .map_err(|e| {
            error!("create_email_verification error: {e}");
            Error::internal("Failed to create the confirmation link.")
        })
    }

    async fn count_live_email_verifications(
        &self,
        user_id: i32,
        now: chrono::DateTime<Utc>,
    ) -> Result<u64, Error> {
        EmailVerificationToken::find()
            .filter(email_verification_token::Column::UserId.eq(user_id))
            .filter(email_verification_token::Column::ExpiresAt.gt(now.fixed_offset()))
            .count(self.db.as_ref())
            .await
            .map_err(|e| {
                error!("count_live_email_verifications error: {e}");
                Error::internal("System error.")
            })
    }

    async fn retire_email_changes(&self, user_id: i32) -> Result<(), Error> {
        EmailVerificationToken::update_many()
            .col_expr(
                email_verification_token::Column::UsedAt,
                Expr::current_timestamp().into(),
            )
            .filter(email_verification_token::Column::UserId.eq(user_id))
            .filter(email_verification_token::Column::UsedAt.is_null())
            .filter(email_verification_token::Column::NewEmail.is_not_null())
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("retire_email_changes error: {e}");
                Error::internal("System error.")
            })
    }

    async fn claim_email_verification(
        &self,
        token_hash: &str,
    ) -> Result<Option<ClaimedEmail>, Error> {
        let fail = |e: DbErr| {
            error!("claim_email_verification error: {e}");
            Error::internal("System error.")
        };
        // The link is spent and the address stamped (and moved to) together,
        // or nothing happens: a failure leaves the link to be opened again.
        let txn = self.db.begin().await.map_err(fail)?;
        let claimed = EmailVerificationToken::update_many()
            .col_expr(
                email_verification_token::Column::UsedAt,
                Expr::current_timestamp().into(),
            )
            .filter(email_verification_token::Column::TokenHash.eq(token_hash))
            .filter(email_verification_token::Column::UsedAt.is_null())
            .filter(
                Expr::col((
                    EmailVerificationToken,
                    email_verification_token::Column::ExpiresAt,
                ))
                .gt(Expr::current_timestamp()),
            )
            .exec_with_returning(&txn)
            .await
            .map_err(fail)?;
        let Some(token) = claimed.into_iter().next() else {
            return Ok(None);
        };
        // The account as it is now, held until the commit: two links opened
        // at once move it one after the other.
        let Some(account) = UserEntity::find_by_id(token.user_id)
            .filter(user::Column::DeletedAt.is_null())
            .lock_exclusive()
            .one(&txn)
            .await
            .map_err(fail)?
        else {
            return Ok(None);
        };
        let moved_to = token.new_email.filter(|to| *to != account.email);
        let mut update = UserEntity::update_many()
            .col_expr(
                user::Column::EmailVerifiedAt,
                Expr::current_timestamp().into(),
            )
            .filter(user::Column::Id.eq(account.id));
        if let Some(to) = &moved_to {
            update = update.col_expr(user::Column::Email, Expr::value(to.clone()));
        }
        // Another account took the address since the link was sent: no form
        // field to put it on, the link's page says it.
        update.exec(&txn).await.map_err(|e| {
            if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                Error::new(ErrorCode::EmailTaken)
            } else {
                fail(e)
            }
        })?;
        if moved_to.is_some() {
            // Every other link went to the old address or names another one:
            // none of them may act on the account now.
            EmailVerificationToken::update_many()
                .col_expr(
                    email_verification_token::Column::UsedAt,
                    Expr::current_timestamp().into(),
                )
                .filter(email_verification_token::Column::UserId.eq(account.id))
                .filter(email_verification_token::Column::UsedAt.is_null())
                .exec(&txn)
                .await
                .map_err(fail)?;
            PasswordResetToken::update_many()
                .col_expr(
                    password_reset_token::Column::UsedAt,
                    Expr::current_timestamp().into(),
                )
                .filter(password_reset_token::Column::UserId.eq(account.id))
                .filter(password_reset_token::Column::UsedAt.is_null())
                .exec(&txn)
                .await
                .map_err(fail)?;
        }
        txn.commit().await.map_err(fail)?;
        let moved = moved_to.is_some();
        Ok(Some(ClaimedEmail {
            user_id: account.id,
            confirmed_previous: (moved && account.email_verified_at.is_some())
                .then(|| account.email.clone()),
            email: moved_to.unwrap_or(account.email),
            moved,
        }))
    }

    async fn record_failed_login(
        &self,
        user_id: i32,
        max_attempts: i32,
        lockout_minutes: i64,
    ) -> Result<(), Error> {
        // One UPDATE: count the attempt and, at the threshold, lock. A lock
        // that has run out starts a fresh count at this attempt; otherwise the
        // count would stay at the threshold and every later typo would lock
        // the account again. Each SET sees the row as it was.
        let locked_until = || Expr::col((user::Entity, user::Column::LockedUntil));
        let lock_ran_out = || locked_until().lte(Expr::current_timestamp());
        // This attempt's number: 1 after a lock ran out, else one more.
        let attempt = || -> SimpleExpr {
            Expr::case(
                locked_until().is_not_null().and(lock_ran_out()),
                Expr::val(1),
            )
            .finally(Expr::col((user::Entity, user::Column::FailedLoginAttempts)).add(1))
            .into()
        };
        let lockout = Expr::val(format!("{lockout_minutes} minutes")).cast_as(PgType::Interval);
        UserEntity::update_many()
            .col_expr(user::Column::FailedLoginAttempts, attempt())
            .col_expr(
                user::Column::LockedUntil,
                Expr::case(
                    Expr::expr(attempt()).gte(max_attempts),
                    Expr::current_timestamp().add(lockout),
                )
                .case(lock_ran_out(), null())
                .finally(locked_until())
                .into(),
            )
            .filter(user::Column::Id.eq(user_id))
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(|e| {
                error!("record_failed_login error: {e}");
                Error::internal("System error.")
            })
    }

    async fn reset_failed_login(&self, user_id: i32) -> Result<(), Error> {
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
                Error::internal("System error.")
            })
    }
}

/// The admin list's criteria: the words in the name or email,
/// and the on / off chip.
fn admin_condition(filter: &AdminFilter) -> sea_orm::Condition {
    admin_condition_base(filter).add(crate::admin_list::created_between(
        (
            crate::entities::user::Entity,
            crate::entities::user::Column::CreatedOn,
        ),
        filter,
    ))
}

fn admin_condition_base(filter: &AdminFilter) -> sea_orm::Condition {
    let mut cond = sea_orm::Condition::all()
        .add(search_condition(ADMIN_USER_SEARCH_COLUMNS, filter.search()))
        .add(switch_state(user::Column::IsActive, filter.state));
    if filter.gap.as_deref() == Some("unverified_email") {
        cond = cond.add(user::Column::EmailVerifiedAt.is_null());
    }
    match filter.role_id {
        Some(id) => cond.add(user::Column::RoleId.eq(id)),
        None => cond,
    }
}

/// `users.email_verified_at IS NOT NULL`.
fn email_verified() -> SimpleExpr {
    Expr::col((user::Entity, user::Column::EmailVerifiedAt)).is_not_null()
}

/// The address the account's newest live link would move it to (unused, not
/// run out, with a `new_email`), as a subquery on the `users` row.
fn pending_email() -> SimpleExpr {
    scalar(
        Query::select()
            .column(email_verification_token::Column::NewEmail)
            .from(EmailVerificationToken)
            .and_where(
                Expr::col((
                    EmailVerificationToken,
                    email_verification_token::Column::UserId,
                ))
                .equals((user::Entity, user::Column::Id)),
            )
            .and_where(Expr::col(email_verification_token::Column::UsedAt).is_null())
            .and_where(Expr::col(email_verification_token::Column::NewEmail).is_not_null())
            .and_where(
                Expr::col(email_verification_token::Column::ExpiresAt)
                    .gt(Expr::current_timestamp()),
            )
            .order_by(email_verification_token::Column::Id, Order::Desc)
            .limit(1)
            .to_owned(),
    )
}
