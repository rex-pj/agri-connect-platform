//! The articles over the JSON API: what the Zalo Mini App reads. The same
//! use case as the pages; gated with them (`features::Feature::Articles`).

use application::{
    outputs::{PagedResponse, PostDto},
    posts::POST_KINDS,
};
use axum::{extract::State, Json};
use domain::{models::post::PostQuery, Error, ErrorCode};
use serde::Deserialize;

use crate::{
    app_state::AppState,
    extract::{ApiPath, ApiQuery},
    pages::parse_id,
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ArticlesQuery {
    /// `news` or `guide`; none: both.
    pub kind: String,
    /// A sector's and every sector's.
    pub sector: String,
    /// A province's and the whole region's.
    pub province: String,
    pub page: Option<i64>,
}

pub struct ArticleHandler;

impl ArticleHandler {
    /// GET /api/explore/articles
    pub async fn list(
        State(state): State<AppState>,
        ApiQuery(q): ApiQuery<ArticlesQuery>,
    ) -> Result<Json<PagedResponse<PostDto>>, Error> {
        let kind = q.kind.trim();
        if !kind.is_empty() && !POST_KINDS.contains(&kind) {
            return Err(Error::field("kind", ErrorCode::InvalidPostKind));
        }
        let non_empty = |s: &str| Some(s.trim().to_owned()).filter(|s| !s.is_empty());
        let query = PostQuery {
            kind: non_empty(kind),
            sector: non_empty(&q.sector),
            province_code: non_empty(&q.province),
            ..PostQuery::default()
        };
        Ok(Json(
            state
                .usecases
                .post
                .public_page(&query, q.page.unwrap_or(1))
                .await?,
        ))
    }

    /// GET /api/explore/articles/{id}
    pub async fn get(
        State(state): State<AppState>,
        ApiPath(raw_id): ApiPath<String>,
    ) -> Result<Json<PostDto>, Error> {
        let id = parse_id(&raw_id, ErrorCode::PostNotFound)?;
        Ok(Json(state.usecases.post.public_get(id).await?))
    }
}
