use application::{authz::require_actor, usecases::media_usecase::file_too_large};
use axum::{
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use domain::{models::file::FileVariant, Error, ErrorCode};
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
        let image = read_field(form, IMAGE_FIELD, max).await?;
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

/// The bytes of the part named `name`; other parts are skipped. Over the body
/// limit is `file_too_large`, so the user learns the cap.
async fn read_field(form: ApiMultipart, name: &str, max: usize) -> Result<Vec<u8>, Error> {
    let malformed = |e: axum::extract::multipart::MultipartError| {
        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            file_too_large(max)
        } else {
            Error::new(ErrorCode::InvalidRequestBody).with_detail(e.body_text())
        }
    };
    let ApiMultipart(mut form) = form;
    while let Some(field) = form.next_field().await.map_err(malformed)? {
        if field.name() == Some(name) {
            return field.bytes().await.map(|b| b.to_vec()).map_err(malformed);
        }
    }
    Err(Error::new(ErrorCode::ImageRequired))
}
