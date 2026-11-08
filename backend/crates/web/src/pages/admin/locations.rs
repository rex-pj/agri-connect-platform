//! /admin/locations and /admin/locations/{id}: the location presets the
//! explore page offers ("Đồng Tháp — Cai Lậy", "An Giang — Long Xuyên"), with a
//! create form, and one preset's edit form, visibility and deletion.
//!
//! Hiding and deleting ask for confirmation and a reason (a dialog with JS,
//! the same form on the page without); showing again is one button. The
//! list's rows carry them all (`?hide={id}` / `?delete={id}` open the
//! confirmation there); sent with `back=list`, they land back on the list.

use application::{
    authz::Actor,
    constants::LOCATION_LABEL_MAX,
    inputs::LocationInput,
    outputs::{AdminLocationDto, AreaGroupDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{
            is_refusal, render_admin, url, Asked, ListPosition, ListReturn, Outcome, RowConfirm,
            BACK_TO_LIST,
        },
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
    /// The ward chosen to fill the form (`w28471`).
    pub ward: String,
}

impl LocationForm {
    fn input(&self) -> Result<LocationInput, Error> {
        Ok(LocationInput {
            label: self.label.clone(),
            lat: coordinate(&self.lat),
            lng: coordinate(&self.lng),
            display_order: whole_number(&self.display_order, "display_order")?,
            ward: Some(self.ward.clone()),
        })
    }

    fn from_dto(l: &AdminLocationDto) -> Self {
        Self {
            label: l.label.clone(),
            lat: l.lat.to_string(),
            lng: l.lng.to_string(),
            display_order: l.display_order.to_string(),
            ward: String::new(),
        }
    }
}

#[derive(Serialize)]
pub struct LocationRow {
    pub location: AdminLocationDto,
    pub href: String,
    pub status_action: String,
    /// The list again with this row's hide / delete confirmation open.
    pub hide_href: String,
    pub delete_href: String,
}

#[derive(Serialize)]
pub struct LocationsPage {
    /// The use case's limit, for the box's `maxlength`.
    pub label_max: usize,
    pub rows: Vec<LocationRow>,
    pub form: LocationForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub confirm: Option<RowConfirm>,
    /// The hidden fields of every row action: land back here.
    pub keep: ListReturn,
    pub flash: Option<Flash>,
    /// The wards the create form fills itself from, by province.
    pub wards: Vec<AreaGroupDto>,
}

/// The list page around the create form; `asked`: the row whose hide or
/// delete confirmation is open.
struct ListView {
    form: LocationForm,
    errors: FormErrors,
    asked: Asked,
    flash: Option<Flash>,
    status: StatusCode,
}

async fn list_page(state: &AppState, actor: &Actor, view: ListView) -> Result<Response, Error> {
    let ListView {
        form,
        errors,
        asked,
        flash,
        status,
    } = view;
    let (locations, wards) = tokio::try_join!(
        state.usecases.admin.list_locations_admin(actor),
        state.usecases.admin.location_choices(actor),
    )?;
    let position = ListPosition::default();
    let mut confirm = None;
    let mut rows = Vec::with_capacity(locations.len());
    for location in locations {
        let id = location.id.to_string();
        let status_action = url("admin_location_status", &[("id", &id)])?;
        if asked.hide == Some(location.id) && location.is_active {
            confirm = Some(RowConfirm {
                kind: "hide",
                name: location.label.clone(),
                action: status_action.clone(),
            });
        }
        if asked.delete == Some(location.id) {
            confirm = Some(RowConfirm {
                kind: "delete",
                name: location.label.clone(),
                action: url("admin_location_delete", &[("id", &id)])?,
            });
        }
        rows.push(LocationRow {
            href: url("admin_location", &[("id", &id)])?,
            status_action,
            hide_href: position.href("admin_locations", &[("hide", &id)])?,
            delete_href: position.href("admin_locations", &[("delete", &id)])?,
            location,
        });
    }
    let path = url("admin_locations", &[])?;
    let body = LocationsPage {
        label_max: LOCATION_LABEL_MAX,
        rows,
        form,
        errors,
        create_action: path.clone(),
        confirm,
        keep: ListReturn::new(position),
        flash,
        wards,
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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    /// The location whose hide / delete confirmation is open.
    pub hide: String,
    pub delete: String,
}

/// GET /admin/locations
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let view = ListView {
        form: LocationForm::default(),
        errors: FormErrors::default(),
        asked: Asked::new(&q.hide, &q.delete),
        flash: flash.0,
        status: StatusCode::OK,
    };
    let mut response = list_page(&state, &actor, view).await?;
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
        state.usecases.admin.create_location(&actor, i).await
    }
    .await;
    match result {
        Ok(created) => redirect_to(created.id, Flash::LocationCreated),
        Err(err) if is_refusal(&err) => {
            let view = ListView {
                errors: FormErrors::from_error(&state, &err),
                form,
                asked: Asked::default(),
                flash: None,
                status: err.kind().status(),
            };
            list_page(&state, &actor, view).await
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
    /// The wards the form fills itself from, by province.
    pub wards: Vec<AreaGroupDto>,
}

async fn location_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<LocationForm>,
) -> Result<Response, Error> {
    let (location, wards) = tokio::try_join!(
        state.usecases.admin.get_location_admin(actor, id),
        state.usecases.admin.location_choices(actor),
    )?;
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
        wards,
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
        state.usecases.admin.update_location(&actor, id, i).await
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
    /// `list` when sent from the list's row: land back there.
    pub back: String,
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
    if form.back == BACK_TO_LIST {
        return ListPosition::default().back_to("admin_locations", flash);
    }
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
