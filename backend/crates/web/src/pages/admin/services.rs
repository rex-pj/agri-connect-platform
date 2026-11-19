//! /admin/services and /admin/services/{id}: the services providers offer.
//!
//! The list can be narrowed to one provider (`?provider_id=`, linked from
//! the provider's page); only then does it show a create form, since a
//! service always belongs to a provider. A service's page edits it, hides it
//! (confirmation with a reason), shows it again (one button) and deletes it
//! (confirmation with a reason). The list's rows carry the same actions
//! (`?hide=` / `?delete=` open their confirmation there); sent with
//! `back=list` and the list position, they land back on that list page.

use application::{
    authz::Actor,
    outputs::ListingActivityDto,
    outputs::{AdminPagedResponse, AdminServiceDto, CategoryDto},
    usecases::admin_usecase::HistoryOf,
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{models::business::ListingRef, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::IMAGE_FIELD,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiMultipart, ApiPath, ApiQuery},
    labels::sector_label,
    middleware::auth::Viewer,
    pages::admin::sector_href,
    pages::{
        admin::{
            history_rows, is_refusal,
            listing_record::{listing_controls, ListingState, SERVICE_WORDS},
            render_admin, url, Asked, Confirm, HistoryRow, ListPosition, ListReturn, Outcome,
            ProviderFilter, ReasonForm, RecordHeader, RowConfirm, SearchBox, BACK_TO_LIST,
        },
        canonical_url,
        dashboard::{max_upload_mb, prepare},
        forms::{reason, FormErrors},
        listing::Pagination,
        parse_id, see_other,
        service_form::{service_categories, Limits, ServiceForm, LIMITS},
    },
};

/// The id of the create sheet on a provider's services list (`services.html`):
/// the provider's page links straight to it.
pub const CREATE_SHEET: &str = "them-dich-vu";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    pub provider_id: String,
    pub q: String,
    /// The service whose hide confirmation is open.
    pub hide: String,
    /// The service whose delete confirmation is open.
    pub delete: String,
}

#[derive(Serialize)]
pub struct ServiceRow {
    pub service: AdminServiceDto,
    pub href: String,
    pub provider_href: String,
    /// Its type's sector's name, "every sector" for none.
    pub sector_label: String,
    pub sector_href: Option<String>,
    /// Its service type's admin page.
    pub category_href: String,
    pub hide_href: String,
    pub delete_href: String,
    pub status_action: String,
}

/// The create form, shown when the list is one provider's.
#[derive(Serialize)]
pub struct CreateForm {
    pub business_id: i32,
    pub action: String,
    pub categories: Vec<CategoryDto>,
    pub form: ServiceForm,
    pub errors: FormErrors,
}

#[derive(Serialize)]
pub struct ServicesPage {
    pub total: i64,
    pub rows: Vec<ServiceRow>,
    pub pagination: Option<Pagination>,
    /// Set when the list is one provider's.
    pub filter: Option<ProviderFilter>,
    pub search: SearchBox,
    pub create: Option<CreateForm>,
    pub limits: &'static Limits,
    pub confirm: Option<RowConfirm>,
    /// The hidden fields of every row action: land back here.
    pub keep: ListReturn,
    pub flash: Option<Flash>,
}

async fn list_page(
    state: &AppState,
    actor: &Actor,
    position: ListPosition,
    asked: Asked,
    create_form: Option<(ServiceForm, FormErrors)>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response, Error> {
    let provider_id = position.provider_id();
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = state
        .usecases
        .admin
        .list_services_admin(actor, position.page(), provider_id, position.search())
        .await?;
    let position = ListPosition {
        page: page.to_string(),
        ..position
    };
    let mut confirm = None;
    let mut rows = Vec::with_capacity(items.len());
    let sectors = state.usecases.sector.named().await?;
    for service in items {
        let id = service.id.to_string();
        let status_action = url("admin_service_status", &[("id", &id)])?;
        if asked.hide == Some(service.id) && service.is_active {
            confirm = Some(RowConfirm {
                kind: "hide",
                name: service.name.clone(),
                action: status_action.clone(),
            });
        }
        if asked.delete == Some(service.id) {
            confirm = Some(RowConfirm {
                kind: "delete",
                name: service.name.clone(),
                action: url("admin_service_delete", &[("id", &id)])?,
            });
        }
        rows.push(ServiceRow {
            href: position.record_href("admin_service", service.id)?,
            sector_label: sector_label(state, &sectors, service.sector.as_deref()),
            sector_href: sector_href(&sectors, service.sector.as_deref())?,
            category_href: url(
                "admin_category",
                &[("id", &service.category_id.to_string())],
            )?,
            hide_href: position.href("admin_services", &[("hide", &id)])?,
            delete_href: position.href("admin_services", &[("delete", &id)])?,
            status_action,
            provider_href: url(
                "admin_provider",
                &[("id", &service.business_id.to_string())],
            )?,
            service,
        });
    }
    let path = url("admin_services", &[])?;
    let create = match provider_id {
        Some(business_id) => {
            let (form, errors) = create_form.unwrap_or_default();
            Some(CreateForm {
                business_id,
                action: path.clone(),
                categories: service_categories(state).await?,
                form,
                errors,
            })
        }
        None => None,
    };
    let name = rows.first().map(|r| r.service.business_name.clone());
    let body = ServicesPage {
        total,
        filter: position.provider_filter("admin_services", name)?,
        search: position.search_box("admin_services")?,
        pagination: position.pagination(&path, page, total_pages),
        rows,
        create,
        limits: &LIMITS,
        confirm,
        keep: ListReturn::new(position),
        flash,
    };
    render_admin(
        state,
        actor,
        &path,
        status,
        "pages/admin/services.html",
        &body,
    )
}

/// GET /admin/services
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let position = ListPosition {
        page: q.page,
        provider_id: q.provider_id,
        q: q.q,
        ..ListPosition::default()
    };
    let asked = Asked::new(&q.hide, &q.delete);
    let mut response = list_page(
        &state,
        &actor,
        position,
        asked,
        None,
        flash.0,
        StatusCode::OK,
    )
    .await?;
    flash.clear_on(&mut response);
    Ok(response)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CreateRequest {
    pub business_id: String,
    pub category_id: String,
    pub name: String,
    pub description: String,
    pub price_note: String,
}

/// POST /admin/services
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(req): ApiForm<CreateRequest>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let business_id = parse_id(&req.business_id, ErrorCode::ProviderNotFound)?;
    let form = ServiceForm {
        category_id: req.category_id,
        name: req.name,
        description: req.description,
        price_note: req.price_note,
    };
    let result = state
        .usecases
        .admin
        .create_service_admin(
            &actor,
            business_id,
            form.category_id(),
            form.name.clone(),
            Some(form.description.clone()),
            Some(form.price_note.clone()),
        )
        .await;
    match result {
        Ok(created) => {
            ListPosition::default().to_record("admin_service", created.id, Flash::ServiceCreated)
        }
        Err(err) if is_refusal(&err) => {
            let errors = FormErrors::from_error(&state, &err);
            let position = ListPosition {
                provider_id: business_id.to_string(),
                ..ListPosition::default()
            };
            let status = err.kind().status();
            list_page(
                &state,
                &actor,
                position,
                Asked::default(),
                Some((form, errors)),
                None,
                status,
            )
            .await
        }
        Err(err) => Err(err),
    }
}

#[derive(Serialize)]
pub struct ServicePage {
    pub service: AdminServiceDto,
    /// Its latest changes: who hid, edited or re-photographed it, and why.
    pub history: Vec<HistoryRow>,
    /// Its taps and contact requests lately.
    pub activity: ListingActivityDto,
    /// Its type's sector's name, "every sector" for none.
    pub sector_label: String,
    pub categories: Vec<CategoryDto>,
    pub form: ServiceForm,
    pub errors: FormErrors,
    pub limits: &'static Limits,
    pub update_action: String,
    /// The photo box's size line.
    pub max_upload_mb: usize,
    pub provider_href: String,
    /// Its service type's admin page, where a hidden type is shown again.
    pub category_href: String,
    /// Its name, state and actions (`listing_record`), and the
    /// confirmations they open.
    pub header: RecordHeader,
    pub confirms: Vec<Confirm>,
    pub flash: Option<Flash>,
}

/// The list a service's page leads back to: the one it was opened from, else
/// its provider's services.
fn back_list(back: &ListPosition, business_id: i32) -> ListPosition {
    if back.is_default() {
        ListPosition {
            provider_id: business_id.to_string(),
            ..ListPosition::default()
        }
    } else {
        back.clone()
    }
}

async fn service_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    back: &ListPosition,
    outcome: Outcome<ServiceForm>,
) -> Result<Response, Error> {
    let admin = &state.usecases.admin;
    let service = admin.get_service_admin(actor, id).await?;
    let id_text = id.to_string();
    let business = service.business_id.to_string();
    let path = url("admin_service", &[("id", &id_text)])?;
    let sectors = state.usecases.sector.named().await?;
    let listing = ListingRef::Service {
        business_id: service.business_id,
        id,
    };
    let public_href = service
        .is_public
        .then(|| canonical_url("service", service.id, &service.name))
        .transpose()?;
    let back_label = if back.is_default() {
        "admin.provider_services"
    } else {
        "admin_menu.services"
    };
    let (header, confirms) = listing_controls(
        state,
        &SERVICE_WORDS,
        ListingState {
            name: &service.name,
            is_active: service.is_active,
            image_file_id: service.image_file_id,
            public_href,
            back_href: back_list(back, service.business_id).href("admin_services", &[])?,
            back_label: state.locale().text(back_label),
            status_action: back.record_href("admin_service_status", id)?,
            delete_action: back.record_href("admin_service_delete", id)?,
            image_delete_action: back.record_href("admin_service_image_delete", id)?,
        },
    )?;
    let body = ServicePage {
        header,
        confirms,
        history: history_rows(state, actor, HistoryOf::Service(id)).await?,
        activity: admin.listing_activity(actor, listing).await?,
        sector_label: sector_label(state, &sectors, service.sector.as_deref()),
        form: outcome.form.unwrap_or_else(|| {
            ServiceForm::stored(
                service.category_id,
                &service.name,
                service.description.as_deref(),
                service.price_note.as_deref(),
            )
        }),
        categories: service_categories(state).await?,
        errors: outcome.errors,
        limits: &LIMITS,
        update_action: back.record_href("admin_service", id)?,
        max_upload_mb: max_upload_mb(state),
        category_href: url(
            "admin_category",
            &[("id", &service.category_id.to_string())],
        )?,
        provider_href: url("admin_provider", &[("id", &business)])?,
        service,
        flash: outcome.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/service.html",
        &body,
    )
}

/// GET /admin/services/{id}; opened from a list, its position rides along
/// (`ListPosition::record_href`).
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiQuery(back): ApiQuery<ListPosition>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ServiceNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let mut response = service_page(&state, &actor, id, &back, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/services/{id} (multipart: the boxes and an optional photo,
/// checked before anything is saved, as on the owner's form)
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiQuery(back): ApiQuery<ListPosition>,
    upload: ApiMultipart,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ServiceNotFound)?;
    // Who is asking, and that the service exists, before reading the photo.
    state.usecases.admin.get_service_admin(&actor, id).await?;
    let max = state.usecases.media.max_upload_bytes();
    let (form, file): (ServiceForm, _) = upload.into_form(IMAGE_FIELD, max).await?;
    let image = match prepare(&state, file).await {
        Ok(image) => image,
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::photo_refused(&state, &err, form);
            return service_page(&state, &actor, id, &back, outcome).await;
        }
        Err(err) => return Err(err),
    };
    let result = state
        .usecases
        .admin
        .update_service_admin(
            &actor,
            id,
            form.category_id(),
            form.name.clone(),
            Some(form.description.clone()),
            Some(form.price_note.clone()),
        )
        .await;
    match result {
        Ok(_) => {
            if let Some(image) = image {
                state
                    .usecases
                    .media
                    .attach_service_image_admin(&actor, id, image)
                    .await?;
            }
            back.to_record("admin_service", id, Flash::ServiceSaved)
        }
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            service_page(&state, &actor, id, &back, outcome).await
        }
        Err(err) => Err(err),
    }
}

/// POST /admin/services/{id}/image/delete: the photo off, the service kept.
pub async fn remove_image(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiQuery(back): ApiQuery<ListPosition>,
    ApiForm(form): ApiForm<ReasonForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ServiceNotFound)?;
    state
        .usecases
        .media
        .remove_service_image(&actor, id, reason(&form.reason))
        .await?;
    back.to_record("admin_service", id, Flash::ImageRemoved)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    /// "true" to show, "false" to hide.
    pub is_active: String,
    pub reason: String,
    /// `list` when sent from the list's row: land back there, at `page` /
    /// `provider_id`.
    pub back: String,
    pub page: String,
    pub provider_id: String,
    pub q: String,
}

/// POST /admin/services/{id}/status: from a list's row back to that list,
/// from its page back to it (with the list it was opened from).
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiQuery(back): ApiQuery<ListPosition>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ServiceNotFound)?;
    let show = form.is_active == "true";
    state
        .usecases
        .admin
        .toggle_service_active(&actor, id, show, reason(&form.reason))
        .await?;
    let flash = if show {
        Flash::ServiceShown
    } else {
        Flash::ServiceHidden
    };
    if form.back == BACK_TO_LIST {
        return ListPosition {
            page: form.page,
            provider_id: form.provider_id,
            q: form.q,
            ..ListPosition::default()
        }
        .back_to("admin_services", flash);
    }
    back.to_record("admin_service", id, flash)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct DeleteForm {
    pub reason: String,
    /// `list` when sent from the list's row: land back there, at `page` /
    /// `provider_id`.
    pub back: String,
    pub page: String,
    pub provider_id: String,
    pub q: String,
}

/// POST /admin/services/{id}/delete: back to the list it was sent from (a
/// row's, or the one its page was opened from), else to the provider's
/// services.
pub async fn delete(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiQuery(back): ApiQuery<ListPosition>,
    ApiForm(form): ApiForm<DeleteForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ServiceNotFound)?;
    let admin = &state.usecases.admin;
    let business_id = admin.get_service_admin(&actor, id).await?.business_id;
    admin
        .delete_service_admin(&actor, id, reason(&form.reason))
        .await?;
    let position = if form.back == BACK_TO_LIST {
        ListPosition {
            page: form.page,
            provider_id: form.provider_id,
            q: form.q,
            ..ListPosition::default()
        }
    } else {
        back_list(&back, business_id)
    };
    position.back_to("admin_services", Flash::ServiceDeleted)
}
