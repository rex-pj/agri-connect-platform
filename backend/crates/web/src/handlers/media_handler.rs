use application::authz::require_actor;
use axum::{
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use domain::{models::file::FileVariant, Error};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    constants::{FILE_CACHE_CONTROL, IMAGE_FIELD},
    extract::{ApiMultipart, ApiPath},
    middleware::auth::MaybeActor,
};

pub struct MediaHandler;

impl MediaHandler {
    /// GET /files/{id}
    pub async fn file(
        State(state): State<AppState>,
        ApiPath(id): ApiPath<Uuid>,
    ) -> Result<Response, Error> {
        serve(&state, id, FileVariant::Main).await
    }

    /// GET /files/{id}/thumb
    pub async fn thumb(
        State(state): State<AppState>,
        ApiPath(id): ApiPath<Uuid>,
    ) -> Result<Response, Error> {
        serve(&state, id, FileVariant::Thumb).await
    }

    /// PUT /api/provider/products/{id}/image — multipart, the file in `image`.
    pub async fn put_product_image(
        MaybeActor(actor): MaybeActor,
        State(state): State<AppState>,
        ApiPath(id): ApiPath<i32>,
        form: ApiMultipart,
    ) -> Result<impl IntoResponse, Error> {
        // Before reading the body: an anonymous caller never gets to upload.
        let actor = require_actor(actor)?;
        let max = state.usecases.media.max_upload_bytes();
        let image = form.into_field(IMAGE_FIELD, max).await?;
        let product = state
            .usecases
            .media
            .store_product_image(&actor, id, image)
            .await?;
        Ok(Json(product))
    }
}

async fn serve(state: &AppState, id: Uuid, variant: FileVariant) -> Result<Response, Error> {
    let file = state.usecases.media.file(id, variant).await?;
    let content_type = HeaderValue::from_str(&file.content_type)
        .map_err(|_| Error::internal(format!("file {id}: stored content type is not a header")))?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(FILE_CACHE_CONTROL),
            ),
        ],
        file.bytes,
    )
        .into_response())
}
