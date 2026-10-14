//! JWT (HS256) adapter for the `TokenService` port. The only file that calls
//! `jsonwebtoken`.

use application::{
    constants::JWT_EXPIRY_HOURS,
    ports::{SessionClaims, TokenService},
};
use chrono::{Duration, Utc};
use domain::{models::role::UserRole, Error, ErrorCode, Result};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// HS256 is only as strong as its key; anything shorter than the hash output
/// is brute-forceable offline from a single captured token.
pub const MIN_SECRET_BYTES: usize = 32;

/// Wire format of the token payload. The SvelteKit app decodes `role` from it
/// until its last page is migrated, so the field names are a contract.
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: i32,
    role: UserRole,
    username: String,
    exp: i64,
}

pub struct JwtTokenService {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl JwtTokenService {
    pub fn new(secret: &str) -> Result<Self> {
        if secret.len() < MIN_SECRET_BYTES {
            return Err(Error::internal(format!(
                "JWT_SECRET must be at least {MIN_SECRET_BYTES} bytes"
            )));
        }
        Ok(Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
        })
    }
}

impl TokenService for JwtTokenService {
    fn mint(&self, user_id: i32, username: &str, role: &UserRole) -> Result<String> {
        let claims = Claims {
            sub: user_id,
            role: role.clone(),
            username: username.to_owned(),
            exp: (Utc::now() + Duration::hours(JWT_EXPIRY_HOURS)).timestamp(),
        };
        encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)
            .map_err(|e| Error::internal(format!("jwt encode: {e}")))
    }

    fn verify(&self, token: &str) -> Result<SessionClaims> {
        // Pinning the algorithm stops a token that names a different one
        // (e.g. `none`) from being accepted.
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.leeway = 0;

        let data = decode::<Claims>(token, &self.decoding, &validation)
            .map_err(|e| Error::new(ErrorCode::InvalidToken).with_detail(e.to_string()))?;

        Ok(SessionClaims {
            user_id: data.claims.sub,
            username: data.claims.username,
            role: data.claims.role,
            expires_at: data.claims.exp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-secret-that-is-at-least-32-bytes-long";

    #[test]
    fn short_secret_is_refused() {
        assert!(JwtTokenService::new("too-short").is_err());
    }

    #[test]
    fn minted_token_round_trips() -> Result<()> {
        let svc = JwtTokenService::new(SECRET)?;
        let token = svc.mint(7, "an", &UserRole::Provider)?;
        let claims = svc.verify(&token)?;
        assert_eq!(
            (claims.user_id, claims.username.as_str(), claims.role),
            (7, "an", UserRole::Provider)
        );
        Ok(())
    }

    #[test]
    fn token_signed_with_another_key_is_invalid() -> Result<()> {
        let token = JwtTokenService::new(SECRET)?.mint(1, "x", &UserRole::Admin)?;
        let other = JwtTokenService::new("another-secret-that-is-also-32-bytes-long")?;
        assert_eq!(
            other.verify(&token).err().map(|e| e.code()),
            Some(ErrorCode::InvalidToken)
        );
        Ok(())
    }

    #[test]
    fn expired_token_is_invalid() -> Result<()> {
        let svc = JwtTokenService::new(SECRET)?;
        let claims = Claims {
            sub: 1,
            role: UserRole::Admin,
            username: "x".into(),
            exp: Utc::now().timestamp() - 1,
        };
        let token = encode(&Header::new(Algorithm::HS256), &claims, &svc.encoding)
            .map_err(|e| Error::internal(e.to_string()))?;
        assert_eq!(
            svc.verify(&token).err().map(|e| e.code()),
            Some(ErrorCode::InvalidToken)
        );
        Ok(())
    }
}
