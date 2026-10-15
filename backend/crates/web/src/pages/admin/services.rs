//! /admin/services and /admin/services/{id}: the services providers offer.
//!
//! The list can be narrowed to one provider (`?provider_id=`, linked from
//! the provider's page); only then does it show a create form, since a
//! service always belongs to a provider. A service's page edits it, hides it
//! (confirmation with a reason), shows it again (one button) and deletes it
//! (confirmation with a reason).

use application::{
    authz::Actor,
    constants::{SERVICE_DESCRIPTION_MAX, SERVICE_NAME_MAX, SERVICE_PRICE_NOTE_MAX},
    outputs::{AdminPagedResponse, AdminServiceDto, CategoryDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{constants::CATEGORY_TYPE_SERVICE, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{is_refusal, render_admin, url, ListPosition, Outcome},
        forms::{reason, FormErrors},
        listing::{parse_opt, Pagination},
        parse_id, see_other,
    },
};

/// The use case's length limits, for the boxes' `maxlength`.
#[derive(Serialize)]
pub struct Limits {
    pub name: usize,
    pub description: usize,
    pub price_note: usize,
}

const LIMITS: Limits = Limits {
    name: SERVICE_NAME_MAX,
    description: SERVICE_DESCRIPTION_MAX,
    price_note: SERVICE_PRICE_NOTE_MAX,
};

/// The form as typed.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ServiceForm {
    pub category_id: String,
    pub name: String,
    pub description: String,
    pub price_note: String,
}

impl ServiceForm {
    fn from_dto(s: &AdminServiceDto) -> Self {
        Self {
            category_id: s.category_id.to_string(),
            name: s.name.clone(),
            description: s.description.clone().unwrap_or_default(),
            price_note: s.price_note.clone().unwrap_or_default(),
        }
    }

    /// An unparsable choice becomes 0, which the use case reports as an
    /// invalid category on that field.
    fn category_id(&self) -> i32 {
        parse_opt(&self.category_id).unwrap_or(0)
    }
}

async fn service_categories(state: &AppState) -> Result<Vec<CategoryDto>, Error> {
    state
        .usecases
        .explore
        .get_categories(Some(CATEGORY_TYPE_SERVICE))
        .await
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    pub provider_id: String,
}

#[derive(Serialize)]
pub struct ServiceRow {
    pub service: AdminServiceDto,
    pub href: String,
    pub provider_href: String,
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
    pub provider_name: Option<String>,
    pub provider_href: Option<String>,
    pub all_href: String,
    pub create: Option<CreateForm>,
    pub limits: &'static Limits,
    pub flash: Option<Flash>,
}

async fn list_page(
    state: &AppState,
    actor: &Actor,
    position: ListPosition,
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
        .list_services_admin(actor, position.page(), provider_id)
        .await?;
    let mut rows = Vec::with_capacity(items.len());
    for service in items {
        rows.push(ServiceRow {
            href: url("admin_service", &[("id", &service.id.to_string())])?,
            provider_href: url(
                "admin_provider",
                &[("id", &service.business_id.to_string())],
            )?,
            service,
        });
    }
    let path = url("admin_services", &[])?;
    let provider_text = provider_id.map(|p| p.to_string());
    let filter: Vec<(&str, &str)> = provider_text
        .as_deref()
        .map(|p| vec![("provider_id", p)])
        .unwrap_or_default();
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
    let body = ServicesPage {
        total,
        provider_name: provider_id
            .and_then(|_| rows.first().map(|r| r.service.business_name.clone())),
        provider_href: provider_text
            .as_deref()
            .map(|p| url("admin_provider", &[("id", p)]))
            .transpose()?,
        pagination: Pagination::new(&path, &filter, page, total_pages),
        rows,
        all_href: path.clone(),
        create,
        limits: &LIMITS,
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
    };
    let mut response = list_page(&state, &actor, position, None, flash.0, StatusCode::OK).await?;
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
        Ok(created) => redirect_to(created.id, Flash::ServiceCreated),
        Err(err) if is_refusal(&err) => {
            let errors = FormErrors::from_error(&state, &err);
            let position = ListPosition {
                provider_id: business_id.to_string(),
                ..ListPosition::default()
            };
            let status = err.kind().status();
            list_page(&state, &actor, position, Some((form, errors)), None, status).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Serialize)]
pub struct ServicePage {
    pub service: AdminServiceDto,
    pub categories: Vec<CategoryDto>,
    pub form: ServiceForm,
    pub errors: FormErrors,
    pub limits: &'static Limits,
    pub update_action: String,
    pub status_action: String,
    pub delete_action: String,
    pub provider_href: String,
    /// The provider's services, where a deletion lands.
    pub provider_services_href: String,
    pub flash: Option<Flash>,
}

async fn service_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<ServiceForm>,
) -> Result<Response, Error> {
    let service = state.usecases.admin.get_service_admin(actor, id).await?;
    let id_text = id.to_string();
    let business = service.business_id.to_string();
    let path = url("admin_service", &[("id", &id_text)])?;
    let body = ServicePage {
        form: outcome
            .form
            .unwrap_or_else(|| ServiceForm::from_dto(&service)),
        categories: service_categories(state).await?,
        errors: outcome.errors,
        limits: &LIMITS,
        update_action: path.clone(),
        status_action: url("admin_service_status", &[("id", &id_text)])?,
        delete_action: url("admin_service_delete", &[("id", &id_text)])?,
        provider_href: url("admin_provider", &[("id", &business)])?,
        provider_services_href: ListPosition {
            provider_id: business,
            ..ListPosition::default()
        }
        .href("admin_services", &[])?,
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

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_service", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// GET /admin/services/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ServiceNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let mut response = service_page(&state, &actor, id, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/services/{id}
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<ServiceForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ServiceNotFound)?;
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
        Ok(_) => redirect_to(id, Flash::ServiceSaved),
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            service_page(&state, &actor, id, outcome).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    /// "true" to show, "false" to hide.
    pub is_active: String,
    pub reason: String,
}

/// POST /admin/services/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
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
    redirect_to(id, flash)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct DeleteForm {
    pub reason: String,
}

/// POST /admin/services/{id}/delete: back to the provider's services.
pub async fn delete(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
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
    ListPosition {
        provider_id: business_id.to_string(),
        ..ListPosition::default()
    }
    .back_to("admin_services", Flash::ServiceDeleted)
}
