//! `ZaloGateway` over the Zalo Graph API: `GET {graph_url}` with the user's
//! `access_token`, the Mini App's token as `code` and the app's
//! `secret_key`, all as headers. The answer is
//! `{"data": {...}, "error": 0, "message": "Success"}`; any other `error`
//! is a refused token (used, expired, of another app, wrong secret).

use std::time::Duration;

use application::ports::ZaloGateway;
use async_trait::async_trait;
use domain::{Error, ErrorCode, Result};
use serde::Deserialize;
use tracing::{error, warn};

pub struct ZaloGraphGateway {
    client: reqwest::Client,
    graph_url: String,
    secret_key: String,
}

impl ZaloGraphGateway {
    /// `graph_url`: the `me/info` endpoint; `timeout`: how long a call may
    /// take before the farmer is told to try again.
    pub fn new(graph_url: String, secret_key: String, timeout: Duration) -> Result<Self> {
        // The app's secret travels as a custom header, which reqwest keeps on
        // a redirect to another host (it strips only the standard credential
        // headers): never follow one, and never speak plain HTTP.
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .build()
            .map_err(|e| Error::internal(format!("zalo http client: {e}")))?;
        Ok(Self {
            client,
            graph_url,
            secret_key,
        })
    }

    async fn data(&self, access_token: &str, code: &str) -> Result<Data> {
        let response = self
            .client
            .get(&self.graph_url)
            .header("access_token", access_token)
            .header("code", code)
            .header("secret_key", &self.secret_key)
            .send()
            .await
            .map_err(|e| {
                error!(error = %e, "zalo graph call failed");
                Error::new(ErrorCode::ZaloUnavailable)
            })?;
        let body: Envelope = response.json().await.map_err(|e| {
            error!(error = %e, "zalo graph answer is not the expected JSON");
            Error::new(ErrorCode::ZaloUnavailable)
        })?;
        parse(body)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Data {
    latitude: Option<String>,
    longitude: Option<String>,
    number: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(default)]
    data: Option<Data>,
    error: i64,
    #[serde(default)]
    message: String,
}

/// The data of a successful answer; a non-zero `error` is a refused token.
fn parse(body: Envelope) -> Result<Data> {
    if body.error != 0 {
        warn!(code = body.error, message = %body.message, "zalo refused a mini app token");
        return Err(Error::new(ErrorCode::ZaloTokenRejected));
    }
    body.data
        .ok_or_else(|| Error::new(ErrorCode::ZaloTokenRejected))
}

/// Zalo writes coordinates as strings ("10.758341").
fn coordinate(value: Option<&str>) -> Result<f64> {
    value
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| Error::new(ErrorCode::ZaloTokenRejected))
}

#[async_trait]
impl ZaloGateway for ZaloGraphGateway {
    async fn location(&self, access_token: &str, code: &str) -> Result<(f64, f64)> {
        let data = self.data(access_token, code).await?;
        Ok((
            coordinate(data.latitude.as_deref())?,
            coordinate(data.longitude.as_deref())?,
        ))
    }

    async fn phone_number(&self, access_token: &str, code: &str) -> Result<String> {
        self.data(access_token, code)
            .await?
            .number
            .filter(|n| !n.trim().is_empty())
            .ok_or_else(|| Error::new(ErrorCode::ZaloTokenRejected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(json: &str) -> Result<Data> {
        serde_json::from_str::<Envelope>(json)
            .map_err(|e| Error::internal(e.to_string()))
            .and_then(parse)
    }

    #[test]
    fn a_location_answer_reads_as_two_numbers() -> Result<()> {
        let data = envelope(
            r#"{"data":{"provider":"gps","latitude":"10.758341","longitude":"106.745863","timestamp":"1666249171003"},"error":0,"message":"Success"}"#,
        )?;
        assert_eq!(coordinate(data.latitude.as_deref())?, 10.758341);
        assert_eq!(coordinate(data.longitude.as_deref())?, 106.745863);
        Ok(())
    }

    #[test]
    fn a_used_token_is_refused() {
        let err = envelope(r#"{"error":119,"message":"code has already been used"}"#).err();
        assert_eq!(err.map(|e| e.code()), Some(ErrorCode::ZaloTokenRejected));
    }
}
