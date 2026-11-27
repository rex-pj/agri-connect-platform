//! What the content editors ask while an admin types: a page's free address,
//! an article's address, a text as it will show. The same use cases as the
//! pages; each checks its own permission.

use application::authz::require_actor;
use axum::{extract::State, response::IntoResponse, Json};
use domain::Error;
use serde::Deserialize;

use crate::{
    app_state::AppState,
    extract::{ApiJson, ApiQuery},
    middleware::auth::MaybeActor,
    requests::SlugSuggestionQuery,
};

/// The text a preview renders.
#[derive(Debug, Deserialize)]
pub struct PreviewRequest {
    #[serde(default)]
    pub text: String,
}

pub struct ContentHandler;

impl ContentHandler {
    /// GET /api/admin/info-pages/slug: a free address for the page form.
    pub async fn info_page_slug(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<SlugSuggestionQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(
            state
                .usecases
                .info_page
                .suggest_code(&actor, &q.text, q.id)
                .await?,
        ))
    }

    /// GET /api/admin/articles/slug: the last part of an article's address.
    pub async fn article_slug(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<SlugSuggestionQuery>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.post.address_slug(&actor, &q.text)?))
    }

    /// POST /api/admin/markdown/preview: a text as the page would show it.
    pub async fn preview(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiJson(req): ApiJson<PreviewRequest>,
    ) -> Result<impl IntoResponse, Error> {
        let actor = require_actor(actor)?;
        Ok(Json(state.usecases.info_page.preview(&actor, &req.text)?))
    }
}
