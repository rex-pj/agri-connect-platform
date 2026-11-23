//! The Zalo Mini App's server calls: the position and the phone behind its
//! `getLocation` / `getPhoneNumber` tokens. Under the public write budget.

use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;

use crate::{app_state::AppState, extract::ApiJson, requests::ZaloTokenRequest};

pub struct ZaloHandler;

impl ZaloHandler {
    /// POST /api/zalo/location — `{access_token, token}` → `{lat, lng}`.
    pub async fn location(
        State(state): State<AppState>,
        ApiJson(req): ApiJson<ZaloTokenRequest>,
    ) -> Result<impl IntoResponse, Error> {
        Ok(Json(
            state
                .usecases
                .zalo
                .location(&req.access_token, &req.token)
                .await?,
        ))
    }

    /// POST /api/zalo/phone — `{access_token, token}` → `{phone}` ("0912345678").
    pub async fn phone(
        State(state): State<AppState>,
        ApiJson(req): ApiJson<ZaloTokenRequest>,
    ) -> Result<impl IntoResponse, Error> {
        Ok(Json(
            state
                .usecases
                .zalo
                .phone(&req.access_token, &req.token)
                .await?,
        ))
    }
}
