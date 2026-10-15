//! /admin/locations and /admin/locations/{id}: the location presets the
//! explore page offers ("Cai Lậy", "Long Xuyên"), with a create form, and one
//! preset's edit form, visibility and deletion.
//!
//! Hiding is reversible and one button; deleting asks for confirmation and a
//! reason (a dialog with JS, the same form on the page without).

use application::{authz::Actor, constants::LOCATION_LABEL_MAX, outputs::AdminLocationDto};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath},
    middleware::auth::Viewer,
    pages::{
        admin::{is_refusal, render_admin, url, Outcome},
        forms::{coordinate, reason, whole_number, FormErrors},
        parse_id, see_other,
    },
};

/// The form as typed: every box a string, so a bad number comes back as it
/// was entered.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct LocationForm {
    pub label: String,
    pub lat: String,
    pub lng: String,
    pub display_order: String,
}

/// What the use case takes, parsed from the form.
struct LocationInput {
    label: String,
    lat: f64,
    lng: f64,
    display_order: i32,
}

impl LocationForm {
    fn input(&self) -> Result<LocationInput, Error> {
        Ok(LocationInput {
            label: self.label.clone(),
            lat: coordinate(&self.lat),
            lng: coordinate(&self.lng),
            display_order: whole_number(&self.display_order, "display_order")?,
        })
    }

    fn from_dto(l: &AdminLocationDto) -> Self {
        Self {
            label: l.label.clone(),
            lat: l.lat.to_string(),
            lng: l.lng.to_string(),
            display_order: l.display_order.to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct LocationRow {
    pub location: AdminLocationDto,
    pub href: String,
}

#[derive(Serialize)]
pub struct LocationsPage {
    /// The use case's limit, for the box's `maxlength`.
    pub label_max: usize,
    pub rows: Vec<LocationRow>,
    pub form: LocationForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub flash: Option<Flash>,
}

async fn list_page(
    state: &AppState,
    actor: &Actor,
    form: LocationForm,
    errors: FormErrors,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response, Error> {
    let locations = state.usecases.admin.list_locations_admin(actor).await?;
    let mut rows = Vec::with_capacity(locations.len());
    for location in locations {
        let href = url("admin_location", &[("id", &location.id.to_string())])?;
        rows.push(LocationRow { location, href });
    }
    let path = url("admin_locations", &[])?;
    let body = LocationsPage {
        label_max: LOCATION_LABEL_MAX,
        rows,
        form,
        errors,
        create_action: path.clone(),
        flash,
    };
    render_admin(
        state,
        actor,
        &path,
        status,
        "pages/admin/locations.html",
        &body,
    )
}

/// GET /admin/locations
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let mut response = list_page(
        &state,
        &actor,
        LocationForm::default(),
        FormErrors::default(),
        flash.0,
        StatusCode::OK,
    )
    .await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/locations
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<LocationForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let result = async {
        let i = form.input()?;
        state
            .usecases
            .admin
            .create_location(&actor, i.label, i.lat, i.lng, i.display_order)
            .await
    }
    .await;
    match result {
        Ok(created) => redirect_to(created.id, Flash::LocationCreated),
        Err(err) if is_refusal(&err) => {
            let errors = FormErrors::from_error(&state, &err);
            list_page(&state, &actor, form, errors, None, err.kind().status()).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Serialize)]
pub struct LocationPage {
    pub label_max: usize,
    pub location: AdminLocationDto,
    pub form: LocationForm,
    pub errors: FormErrors,
    pub update_action: String,
    pub status_action: String,
    pub delete_action: String,
    /// The delete dialog re-opens after a refusal.
    pub confirm_open: bool,
    pub flash: Option<Flash>,
}

async fn location_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<LocationForm>,
) -> Result<Response, Error> {
    let location = state.usecases.admin.get_location_admin(actor, id).await?;
    let id_text = id.to_string();
    let path = url("admin_location", &[("id", &id_text)])?;
    let body = LocationPage {
        label_max: LOCATION_LABEL_MAX,
        form: outcome
            .form
            .unwrap_or_else(|| LocationForm::from_dto(&location)),
        location,
        errors: outcome.errors,
        update_action: path.clone(),
        status_action: url("admin_location_status", &[("id", &id_text)])?,
        delete_action: url("admin_location_delete", &[("id", &id_text)])?,
        confirm_open: outcome.confirm_open,
        flash: outcome.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/location.html",
        &body,
    )
}

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_location", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// GET /admin/locations/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::LocationNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let mut response = location_page(&state, &actor, id, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/locations/{id}
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<LocationForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::LocationNotFound)?;
    let result = async {
        let i = form.input()?;
        state
            .usecases
            .admin
            .update_location(&actor, id, i.label, i.lat, i.lng, i.display_order)
            .await
    }
    .await;
    match result {
        Ok(_) => redirect_to(id, Flash::LocationSaved),
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            location_page(&state, &actor, id, outcome).await
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

/// POST /admin/locations/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::LocationNotFound)?;
    let show = form.is_active == "true";
    state
        .usecases
        .admin
        .toggle_location_active(&actor, id, show, reason(&form.reason))
        .await?;
    let flash = if show {
        Flash::LocationShown
    } else {
        Flash::LocationHidden
    };
    redirect_to(id, flash)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct DeleteForm {
    pub reason: String,
}

/// POST /admin/locations/{id}/delete
pub async fn delete(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<DeleteForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::LocationNotFound)?;
    let result = state
        .usecases
        .admin
        .delete_location(&actor, id, reason(&form.reason))
        .await;
    match result {
        Ok(()) => {
            let mut response = see_other(&url("admin_locations", &[])?)?;
            Flash::LocationDeleted.set_on(&mut response);
            Ok(response)
        }
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, None, true);
            location_page(&state, &actor, id, outcome).await
        }
        Err(err) => Err(err),
    }
}
