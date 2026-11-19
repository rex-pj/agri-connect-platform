//! The Zalo Mini App's two server calls: the position behind a
//! `getLocation` token and the phone behind a `getPhoneNumber` token, each
//! converted with the app's secret key (`ZaloGateway`). Without a secret
//! configured, both answer `zalo_not_configured`; the Mini App then asks for
//! an area or a typed phone instead.

use std::sync::Arc;

use domain::{Error, ErrorCode, Result};

use crate::{
    outputs::{ZaloLocationDto, ZaloPhoneDto},
    ports::ZaloGateway,
    validation::{is_valid_vn_phone, validate_coordinates, VN_CALLING_CODE, VN_PHONE_DIGITS},
};

#[derive(Clone)]
pub struct ZaloUseCase {
    gateway: Option<Arc<dyn ZaloGateway>>,
}

impl ZaloUseCase {
    /// `gateway`: `None` when `ZALO_APP_SECRET` is not set.
    pub fn new(gateway: Option<Arc<dyn ZaloGateway>>) -> Self {
        Self { gateway }
    }

    fn gateway(&self) -> Result<&Arc<dyn ZaloGateway>> {
        self.gateway
            .as_ref()
            .ok_or_else(|| Error::new(ErrorCode::ZaloNotConfigured))
    }

    /// Both tokens must be there before Zalo is called at all.
    fn present(access_token: &str, token: &str) -> Result<()> {
        if access_token.trim().is_empty() || token.trim().is_empty() {
            return Err(Error::new(ErrorCode::ZaloTokenRejected));
        }
        Ok(())
    }

    pub async fn location(&self, access_token: &str, token: &str) -> Result<ZaloLocationDto> {
        let gateway = self.gateway()?;
        Self::present(access_token, token)?;
        let (lat, lng) = gateway.location(access_token.trim(), token.trim()).await?;
        validate_coordinates(lat, lng)?;
        Ok(ZaloLocationDto { lat, lng })
    }

    pub async fn phone(&self, access_token: &str, token: &str) -> Result<ZaloPhoneDto> {
        let gateway = self.gateway()?;
        Self::present(access_token, token)?;
        let number = gateway
            .phone_number(access_token.trim(), token.trim())
            .await?;
        let phone = local_phone(&number).ok_or_else(|| Error::new(ErrorCode::InvalidPhone))?;
        Ok(ZaloPhoneDto { phone })
    }
}

/// A Vietnamese mobile number as our forms take it ("0912345678"), from
/// the international form Zalo gives ("84912345678", "+84 912 345 678"); a
/// number of another country is `None`.
pub fn local_phone(number: &str) -> Option<String> {
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    let local = match digits.strip_prefix(VN_CALLING_CODE) {
        Some(rest) if rest.len() == VN_PHONE_DIGITS - 1 => format!("0{rest}"),
        _ => digits,
    };
    is_valid_vn_phone(&local).then_some(local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zalo_number_becomes_a_local_one() {
        assert_eq!(
            local_phone("849123456789").as_deref(),
            None,
            "one digit too many"
        );
        assert_eq!(local_phone("84912345678").as_deref(), Some("0912345678"));
        assert_eq!(
            local_phone("+84 912 345 678").as_deref(),
            Some("0912345678")
        );
        assert_eq!(local_phone("0912345678").as_deref(), Some("0912345678"));
        assert_eq!(local_phone("14155550100"), None);
    }

    #[tokio::test]
    async fn without_a_secret_the_calls_say_so() {
        let zalo = ZaloUseCase::new(None);
        let err = zalo.location("a", "b").await.err();
        assert_eq!(err.map(|e| e.code()), Some(ErrorCode::ZaloNotConfigured));
    }
}
