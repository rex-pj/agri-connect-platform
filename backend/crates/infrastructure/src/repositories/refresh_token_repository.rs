use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    models::session::{NewRefreshToken, RefreshToken},
    repositories::refresh_token_repository_trait::RefreshTokenRepositoryTrait,
    Error,
};
use sea_orm::{sea_query::Expr, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::entities::refresh_token::{self, Entity as RefreshTokens};

pub struct RefreshTokenRepository {
    db: Arc<DatabaseConnection>,
}

impl RefreshTokenRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

fn fail(op: &str) -> impl Fn(sea_orm::DbErr) -> Error + '_ {
    move |e| Error::internal(format!("refresh_tokens {op}: {e}"))
}

#[async_trait]
impl RefreshTokenRepositoryTrait for RefreshTokenRepository {
    async fn insert(&self, t: &NewRefreshToken) -> Result<(), Error> {
        let row = refresh_token::ActiveModel {
            id: Set(t.id),
            family_id: Set(t.family_id),
            user_id: Set(t.user_id),
            token_hash: Set(t.token_hash.clone()),
            expires_at: Set(t.expires_at.into()),
            ..Default::default()
        };
        RefreshTokens::insert(row)
            .exec_without_returning(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(fail("insert"))
    }

    async fn find_by_hash(&self, token_hash: &str) -> Result<Option<RefreshToken>, Error> {
        RefreshTokens::find()
            .filter(refresh_token::Column::TokenHash.eq(token_hash))
            .one(self.db.as_ref())
            .await
            .map(|m| {
                m.map(|m| RefreshToken {
                    id: m.id,
                    family_id: m.family_id,
                    user_id: m.user_id,
                    expires_at: m.expires_at.with_timezone(&Utc),
                    used_at: m.used_at.map(|t| t.with_timezone(&Utc)),
                    revoked_at: m.revoked_at.map(|t| t.with_timezone(&Utc)),
                })
            })
            .map_err(fail("find"))
    }

    async fn claim(&self, id: Uuid) -> Result<bool, Error> {
        // One UPDATE with the conditions in WHERE: of two concurrent
        // rotations, the database lets exactly one change the row.
        RefreshTokens::update_many()
            .col_expr(
                refresh_token::Column::UsedAt,
                Expr::current_timestamp().into(),
            )
            .filter(refresh_token::Column::Id.eq(id))
            .filter(refresh_token::Column::UsedAt.is_null())
            .filter(refresh_token::Column::RevokedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected == 1)
            .map_err(fail("claim"))
    }

    async fn revoke_family(&self, family_id: Uuid) -> Result<(), Error> {
        RefreshTokens::update_many()
            .col_expr(
                refresh_token::Column::RevokedAt,
                Expr::current_timestamp().into(),
            )
            .filter(refresh_token::Column::FamilyId.eq(family_id))
            .filter(refresh_token::Column::RevokedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(fail("revoke_family"))
    }

    async fn revoke_user(&self, user_id: i32) -> Result<(), Error> {
        RefreshTokens::update_many()
            .col_expr(
                refresh_token::Column::RevokedAt,
                Expr::current_timestamp().into(),
            )
            .filter(refresh_token::Column::UserId.eq(user_id))
            .filter(refresh_token::Column::RevokedAt.is_null())
            .exec(self.db.as_ref())
            .await
            .map(|_| ())
            .map_err(fail("revoke_user"))
    }

    async fn delete_expired(&self, before: DateTime<Utc>) -> Result<u64, Error> {
        RefreshTokens::delete_many()
            .filter(refresh_token::Column::ExpiresAt.lt(before))
            .exec(self.db.as_ref())
            .await
            .map(|r| r.rows_affected)
            .map_err(fail("delete_expired"))
    }
}
