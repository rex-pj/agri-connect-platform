//! Extractors whose rejections are `domain::Error`, so a malformed body, path
//! or query gets the same coded, localised error body as everything else.
//! Using axum's own `Json`/`Path`/`Query` in a handler would leak axum's
//! plain-text rejections; the convention test in `tests` scans for that.

use application::usecases::media_usecase::file_too_large;
use axum::{
    extract::{FromRequest, FromRequestParts, OptionalFromRequest, Request},
    http::request::Parts,
};
use domain::{Error, ErrorCode};
use serde::de::DeserializeOwned;

pub struct ApiJson<T>(pub T);

impl<T, S> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        <axum::Json<T> as FromRequest<S>>::from_request(req, state)
            .await
            .map(|axum::Json(v)| Self(v))
            .map_err(Error::from)
    }
}

/// For endpoints whose body is optional (e.g. an audit reason): absent when
/// the request has no `Content-Type`, an error when it has a malformed one.
impl<T, S> OptionalFromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        <axum::Json<T> as OptionalFromRequest<S>>::from_request(req, state)
            .await
            .map(|opt| opt.map(|axum::Json(v)| Self(v)))
            .map_err(Error::from)
    }
}

pub struct ApiPath<T>(pub T);

impl<T, S> FromRequestParts<S> for ApiPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(v)| Self(v))
            .map_err(Error::from)
    }
}

pub struct ApiQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ApiQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Query(v)| Self(v))
            .map_err(Error::from)
    }
}

/// `multipart/form-data`, for uploads. A request of another type is refused
/// with `invalid_request_body`.
pub struct ApiMultipart(pub axum::extract::Multipart);

impl<S> FromRequest<S> for ApiMultipart
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        <axum::extract::Multipart as FromRequest<S>>::from_request(req, state)
            .await
            .map(Self)
            .map_err(Error::from)
    }
}

impl ApiMultipart {
    /// The bytes of the part named `name`; other parts are skipped. Over
    /// `max` bytes is `file_too_large`, so the user learns the cap.
    pub async fn into_field(self, name: &str, max: usize) -> Result<Vec<u8>, Error> {
        let mut form = self.0;
        while let Some(field) = form
            .next_field()
            .await
            .map_err(|e| multipart_error(e, max))?
        {
            if field.name() == Some(name) {
                return field
                    .bytes()
                    .await
                    .map(|b| b.to_vec())
                    .map_err(|e| multipart_error(e, max));
            }
        }
        Err(Error::new(ErrorCode::ImageRequired))
    }

    /// A page form with a file box: the text boxes as `T` (string fields with
    /// `#[serde(default)]`, like an `ApiForm` struct), and the file in
    /// `file_field`, `None` when no file was chosen (a browser then sends an
    /// empty part).
    pub async fn into_form<T: DeserializeOwned>(
        self,
        file_field: &str,
        max: usize,
    ) -> Result<(T, Option<Vec<u8>>), Error> {
        let (fields, mut files) = self.into_form_files(&[file_field], max).await?;
        Ok((fields, files.pop().map(|(_, bytes)| bytes)))
    }

    /// `into_form` with several file boxes (`file_fields`): each file chosen
    /// as (its box's name, its bytes), in the order sent; a box left empty
    /// is left out.
    pub async fn into_form_files<T: DeserializeOwned>(
        self,
        file_fields: &[&str],
        max: usize,
    ) -> Result<(T, Vec<(String, Vec<u8>)>), Error> {
        let mut form = self.0;
        let mut text = serde_json::Map::new();
        let mut files = Vec::new();
        while let Some(field) = form
            .next_field()
            .await
            .map_err(|e| multipart_error(e, max))?
        {
            let Some(name) = field.name().map(str::to_owned) else {
                continue;
            };
            if file_fields.contains(&name.as_str()) {
                let bytes = field.bytes().await.map_err(|e| multipart_error(e, max))?;
                if !bytes.is_empty() {
                    files.push((name, bytes.to_vec()));
                }
            } else {
                let value = field.text().await.map_err(|e| multipart_error(e, max))?;
                text.insert(name, serde_json::Value::String(value));
            }
        }
        let fields = serde_json::from_value(serde_json::Value::Object(text))
            .map_err(|e| Error::new(ErrorCode::InvalidRequestBody).with_detail(e.to_string()))?;
        Ok((fields, files))
    }
}

fn multipart_error(e: axum::extract::multipart::MultipartError, max: usize) -> Error {
    if e.status() == axum::http::StatusCode::PAYLOAD_TOO_LARGE {
        file_too_large(max)
    } else {
        Error::new(ErrorCode::InvalidRequestBody).with_detail(e.body_text())
    }
}

/// `application/x-www-form-urlencoded`, for page forms that work without JS.
pub struct ApiForm<T>(pub T);

impl<T, S> FromRequest<S> for ApiForm<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        <axum::Form<T> as FromRequest<S>>::from_request(req, state)
            .await
            .map(|axum::Form(v)| Self(v))
            .map_err(Error::from)
    }
}
