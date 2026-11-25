//! `MapTiles` over an OpenStreetMap-style tile host: `GET` its URL template
//! (`{z}`, `{x}`, `{y}` filled in) with a User-Agent naming this site, as
//! the OpenStreetMap tile usage policy asks. The answer is passed on as it
//! came; caching is the browser's and Cloudflare's (the route's
//! `Cache-Control`), never a copy kept here.

use std::time::Duration;

use application::ports::MapTiles;
use async_trait::async_trait;
use domain::{Error, ErrorCode, Result};
use reqwest::{header, StatusCode};
use tracing::{error, warn};

/// A tile is a 256 px PNG of a few dozen kilobytes; anything far larger is
/// not one.
const TILE_MAX_BYTES: usize = 1024 * 1024;
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

pub struct OsmMapTiles {
    client: reqwest::Client,
    url_template: String,
}

impl OsmMapTiles {
    /// `url_template`: `https://tile.openstreetmap.org/{z}/{x}/{y}.png` or a
    /// paid host's; `user_agent`: the site's name and address, so the host
    /// can tell who to contact; `timeout`: how long a tile may take.
    pub fn new(url_template: String, user_agent: &str, timeout: Duration) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(user_agent)
            .https_only(true)
            .build()
            .map_err(|e| Error::internal(format!("map tile http client: {e}")))?;
        Ok(Self {
            client,
            url_template,
        })
    }

    fn url(&self, z: u8, x: u32, y: u32) -> String {
        self.url_template
            .replace("{z}", &z.to_string())
            .replace("{x}", &x.to_string())
            .replace("{y}", &y.to_string())
    }
}

#[async_trait]
impl MapTiles for OsmMapTiles {
    async fn tile(&self, z: u8, x: u32, y: u32) -> Result<Vec<u8>> {
        let unavailable = |what: &str, detail: String| {
            error!(z, x, y, detail = %detail, "map tile {what}");
            Error::new(ErrorCode::MapUnavailable)
        };
        let response = self
            .client
            .get(self.url(z, x, y))
            .header(header::ACCEPT, "image/png")
            .send()
            .await
            .map_err(|e| unavailable("fetch failed", e.to_string()))?;
        match response.status() {
            StatusCode::OK => {}
            StatusCode::NOT_FOUND => return Err(Error::new(ErrorCode::MapTileNotFound)),
            status => {
                warn!(z, x, y, %status, "map tile host refused");
                return Err(Error::new(ErrorCode::MapUnavailable));
            }
        }
        if response
            .content_length()
            .is_some_and(|n| n > TILE_MAX_BYTES as u64)
        {
            return Err(unavailable("too large", String::new()));
        }
        let body = response
            .bytes()
            .await
            .map_err(|e| unavailable("read failed", e.to_string()))?;
        if body.len() > TILE_MAX_BYTES || !body.starts_with(PNG_SIGNATURE) {
            return Err(unavailable("is not a PNG", format!("{} bytes", body.len())));
        }
        Ok(body.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_template_is_filled_with_the_tile() -> Result<()> {
        let tiles = OsmMapTiles::new(
            "https://tile.example/{z}/{x}/{y}.png".into(),
            "test",
            Duration::from_secs(1),
        )?;
        assert_eq!(
            tiles.url(16, 52_084, 30_506),
            "https://tile.example/16/52084/30506.png"
        );
        Ok(())
    }
}
