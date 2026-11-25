//! JWT (HS256) adapter for the `TokenService` port. The only file that calls
//! `jsonwebtoken`.

use application::{
    constants::JWT_EXPIRY_HOURS,
    ports::{SessionClaims, TokenService},
};
use chrono::{Duration, Utc};
use domain::{Error, ErrorCode, Result};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// HS256 is only as strong as its key; anything shorter than the hash output
/// is brute-forceable offline from a single captured token.
pub const MIN_SECRET_BYTES: usize = 32;

/// Wire format of the token payload. Unknown claims are ignored.
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: i32,
    exp: i64,
    #[serde(default)]
    iat: i64,
    /// The session (refresh-token family). Required: a token without one
    /// could not be ended before it expires, so it is not accepted.
    sid: String,
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
    fn mint(&self, user_id: i32, session: Uuid) -> Result<String> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id,
            exp: (now + Duration::hours(JWT_EXPIRY_HOURS)).timestamp(),
            iat: now.timestamp(),
            sid: session.to_string(),
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

        // A `sid` that is not a session id is a token we never minted.
        let session = Uuid::parse_str(&data.claims.sid)
            .map_err(|e| Error::new(ErrorCode::InvalidToken).with_detail(e.to_string()))?;
        Ok(SessionClaims {
            user_id: data.claims.sub,
            expires_at: data.claims.exp,
            issued_at: data.claims.iat,
            session,
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
        let session = Uuid::new_v4();
        let claims = svc.verify(&svc.mint(7, session)?)?;
        assert_eq!(claims.user_id, 7);
        assert_eq!(claims.session, session);
        Ok(())
    }

    #[test]
    fn token_signed_with_another_key_is_invalid() -> Result<()> {
        let token = JwtTokenService::new(SECRET)?.mint(1, Uuid::new_v4())?;
        let other = JwtTokenService::new("another-secret-that-is-also-32-bytes-long")?;
        assert_eq!(
            other.verify(&token).err().map(|e| e.code()),
            Some(ErrorCode::InvalidToken)
        );
        Ok(())
    }

    /// Signed with our key but in no session (as the JSON login once
    /// minted): nothing could end it, so it is refused.
    #[test]
    fn a_token_without_a_session_is_invalid() -> Result<()> {
        #[derive(Serialize)]
        struct Sessionless {
            sub: i32,
            exp: i64,
            iat: i64,
        }
        let svc = JwtTokenService::new(SECRET)?;
        let now = Utc::now().timestamp();
        let claims = Sessionless {
            sub: 3,
            exp: now + 60,
            iat: now,
        };
        let token = encode(&Header::new(Algorithm::HS256), &claims, &svc.encoding)
            .map_err(|e| Error::internal(e.to_string()))?;
        assert_eq!(
            svc.verify(&token).err().map(|e| e.code()),
            Some(ErrorCode::InvalidToken)
        );
        Ok(())
    }

    #[test]
    fn expired_token_is_invalid() -> Result<()> {
        let svc = JwtTokenService::new(SECRET)?;
        let claims = Claims {
            sub: 1,
            exp: Utc::now().timestamp() - 1,
            iat: Utc::now().timestamp() - 3601,
            sid: Uuid::new_v4().to_string(),
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
