//! The provider's own services: /provider/services/new,
//! /provider/services/{id}/edit and …/delete.
//!
//! Like the product forms these are `multipart/form-data` because they carry
//! an optional photo, and work without JS. The photo is checked before
//! anything is saved, and on an edit the service must be the actor's before
//! the body is read. Visibility is not on the form: a service an admin hid
//! stays hidden whatever the provider saves.

use application::{
    authz::Actor,
    catalogue::{PERM_OWN_SERVICE_DELETE, PERM_OWN_SERVICE_UPDATE},
    outputs::{CategoryDto, ProviderServiceDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::Serialize;

use crate::{
    app_state::AppState,
    constants::IMAGE_FIELD,
    cookies::Flash,
    extract::{ApiMultipart, ApiPath},
    middleware::auth::Viewer,
    pages::{
        admin::is_refusal,
        dashboard::{max_upload_mb, prepare, to_dashboard, url},
        forms::FormErrors,
        parse_id, see_other,
        service_form::{service_categories, Limits, ServiceForm, LIMITS},
    },
    shell::Shell,
    views::render_page_status,
};

#[derive(Serialize)]
pub struct ServicePage {
    /// `None` on the create page.
    pub service: Option<ProviderServiceDto>,
    pub categories: Vec<CategoryDto>,
    pub form: ServiceForm,
    pub errors: FormErrors,
    pub limits: &'static Limits,
    pub action: String,
    /// The save button: always on the create page (the use case decides),
    /// on an edit page only with permission to edit.
    pub can_save: bool,
    pub delete_action: Option<String>,
    pub dashboard_href: String,
    pub max_upload_mb: usize,
}

async fn service_page(
    state: &AppState,
    shell: &Shell,
    actor: &Actor,
    service: Option<ProviderServiceDto>,
    typed: Option<(ServiceForm, FormErrors)>,
    status: StatusCode,
) -> Result<Response, Error> {
    let (action, can_save, delete_action) = match &service {
        Some(s) => {
            let id = s.id.to_string();
            (
                url("provider_service_edit", &[("id", &id)])?,
                actor.can(PERM_OWN_SERVICE_UPDATE),
                actor
                    .can(PERM_OWN_SERVICE_DELETE)
                    .then(|| url("provider_service_delete", &[("id", &id)]))
                    .transpose()?,
            )
        }
        None => (url("provider_service_new", &[])?, true, None),
    };
    let (form, errors) = typed.unwrap_or_else(|| {
        let form = service
            .as_ref()
            .map(|s| {
                ServiceForm::stored(
                    s.category_id,
                    &s.name,
                    s.description.as_deref(),
                    s.price_note.as_deref(),
                )
            })
            .unwrap_or_default();
        (form, FormErrors::default())
    });
    let body = ServicePage {
        categories: service_categories(state).await?,
        service,
        form,
        errors,
        limits: &LIMITS,
        action,
        can_save,
        delete_action,
        dashboard_href: url("provider_dashboard", &[])?,
        max_upload_mb: max_upload_mb(state),
    };
    render_page_status(state, shell, status, "pages/dashboard_service.html", &body)
}

/// A refused form: re-shown with the typed values (a file box never keeps
/// its file, so the photo must be chosen again).
async fn refused(
    state: &AppState,
    shell: &Shell,
    actor: &Actor,
    service: Option<ProviderServiceDto>,
    form: ServiceForm,
    err: Error,
    errors: FormErrors,
) -> Result<Response, Error> {
    if !is_refusal(&err) {
        return Err(err);
    }
    let status = err.kind().status();
    service_page(state, shell, actor, service, Some((form, errors)), status).await
}

/// GET /provider/services/new
pub async fn new_service(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    service_page(&state, &shell, &actor, None, None, StatusCode::OK).await
}

/// POST /provider/services/new (multipart)
pub async fn create_service(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    upload: ApiMultipart,
) -> Result<Response, Error> {
    // Before reading the body: only an account that may list one uploads
    // (signed in, the permission, a business), so no one else gets a photo
    // decoded.
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    state
        .usecases
        .provider_dashboard
        .may_create_service(&actor)
        .await?;
    let max = state.usecases.media.max_upload_bytes();
    let (form, file): (ServiceForm, _) = upload.into_form(IMAGE_FIELD, max).await?;
    let image = match prepare(&state, file).await {
        Ok(image) => image,
        Err(err) => {
            let errors = FormErrors::on_field(&state, &err, IMAGE_FIELD);
            return refused(&state, &shell, &actor, None, form, err, errors).await;
        }
    };
    let created = state
        .usecases
        .provider_dashboard
        .create_service(&actor, form.input(), image)
        .await;
    if let Err(err) = created {
        let errors = FormErrors::from_error(&state, &err);
        return refused(&state, &shell, &actor, None, form, err, errors).await;
    }
    to_dashboard(Flash::ServiceCreated)
}

/// GET /provider/services/{id}/edit
pub async fn edit_service(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ServiceNotFound)?;
    let service = state
        .usecases
        .provider_dashboard
        .get_service(&actor, id)
        .await?;
    service_page(&state, &shell, &actor, Some(service), None, StatusCode::OK).await
}

/// POST /provider/services/{id}/edit (multipart)
pub async fn update_service(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    upload: ApiMultipart,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ServiceNotFound)?;
    let dashboard = &state.usecases.provider_dashboard;
    // Ownership before reading or decoding the photo.
    let current = dashboard.get_service(&actor, id).await?;
    let max = state.usecases.media.max_upload_bytes();
    let (form, file): (ServiceForm, _) = upload.into_form(IMAGE_FIELD, max).await?;
    let media = &state.usecases.media;
    // Uploaded before anything is saved, shown once the boxes are (see the
    // supply form).
    let staged = async {
        match prepare(&state, file).await? {
            Some(image) => media.stage_service_image(&actor, id, image).await.map(Some),
            None => Ok(None),
        }
    }
    .await;
    let staged = match staged {
        Ok(staged) => staged,
        Err(err) => {
            let errors = FormErrors::on_field(&state, &err, IMAGE_FIELD);
            return refused(&state, &shell, &actor, Some(current), form, err, errors).await;
        }
    };
    if let Err(err) = dashboard.update_service(&actor, id, form.input()).await {
        if let Some(staged) = staged {
            media.abandon_staged(staged).await;
        }
        let errors = FormErrors::from_error(&state, &err);
        return refused(&state, &shell, &actor, Some(current), form, err, errors).await;
    }
    if let Some(staged) = staged {
        media.commit_staged(&actor, staged).await?;
    }
    to_dashboard(Flash::ServiceSaved)
}

/// POST /provider/services/{id}/delete
pub async fn delete_service(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ServiceNotFound)?;
    state
        .usecases
        .provider_dashboard
        .delete_service(&actor, id)
        .await?;
    to_dashboard(Flash::ServiceDeleted)
}
