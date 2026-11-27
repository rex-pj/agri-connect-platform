//! /admin/locations: the location presets the explore page offers ("Đồng
//! Tháp — Cai Lậy", "An Giang — Long Xuyên"), with a create sheet and a
//! preset's edit sheet (`EditSheet`, `?edit={id}`: a name, a ward, a point,
//! no page of its own; its former address `/admin/locations/{id}` opens it).
//!
//! Hiding and deleting ask for confirmation and a reason (a dialog with JS,
//! the same form on the page without); showing again is one button. The
//! list's rows carry them all (`?hide={id}` / `?delete={id}` open the
//! confirmation there) and land back on the list; a deletion refused (an
//! area still in use) says why over it.
//!
//! "Add several" is a second sheet: a province chosen (`?bulk=p82`, a GET
//! form that sends itself with JS), its wards as boxes to tick, those a
//! preset already names said so and not offered; posted to
//! `/admin/locations/bulk`, each ticked ward becomes a preset as the one-ward
//! form makes it, and the list opens newest first.

use std::collections::{HashMap, HashSet};

use application::{
    authz::Actor,
    constants::{LOCATION_BULK_MAX, LOCATION_LABEL_MAX},
    inputs::LocationInput,
    outputs::{AdminLocationDto, AreaGroupDto},
    usecases::admin_usecase::HistoryOf,
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::models::admin_list::AdminSort;
use domain::{models::admin_list::LISTING_LIST_STATES, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::MAP_POINT_URL,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{
            history_rows, is_refusal, render_admin, url, AdminTab, Asked, EditSheet, ListPosition,
            ListReturn, RowConfirm, SearchBox, Typed, CATALOGUE_SORTS,
        },
        forms::{coordinate, reason, whole_number, FormErrors},
        listing::{past_the_end, Pagination},
        moved_permanently, parse_id, see_other,
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
    /// The list again with its edit sheet open.
    pub edit_href: String,
    pub status_action: String,
    /// The list again with this row's hide / delete confirmation open.
    pub hide_href: String,
    pub delete_href: String,
}

/// A preset in its edit sheet, with its point on a map.
#[derive(Serialize)]
pub struct EditedLocation {
    pub location: AdminLocationDto,
    pub map_href: String,
}

#[derive(Serialize)]
pub struct LocationsPage {
    /// The shown / hidden chips.
    pub states: Vec<AdminTab>,
    /// The use case's limit, for the box's `maxlength`.
    pub label_max: usize,
    pub rows: Vec<LocationRow>,
    pub form: LocationForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub confirm: Option<RowConfirm>,
    /// The preset whose edit sheet is open.
    pub edit: Option<EditSheet<EditedLocation, LocationForm>>,
    /// Why a deletion was refused (an area still in use).
    pub refusal: Option<String>,
    pub search: SearchBox,
    /// The hidden fields of every row action: land back here.
    pub keep: ListReturn,
    /// Every preset, for the header's count.
    pub total: i64,
    pub pagination: Option<Pagination>,
    pub flash: Option<Flash>,
    /// The wards the create form fills itself from, by province.
    pub wards: Vec<AreaGroupDto>,
    /// The "add several" sheet.
    pub bulk: BulkSheet,
}

/// A ticked ward's box name: `ward.w28439`.
const WARD_PREFIX: &str = "ward.";

/// The "add several" sheet: the province chosen and its wards.
#[derive(Serialize)]
pub struct BulkSheet {
    /// Opened on load: a province was chosen, or the form came back refused.
    pub open: bool,
    /// `p82`, empty before one is chosen.
    pub province: String,
    pub province_name: String,
    pub wards: Vec<BulkWardRow>,
    /// Its wards no preset names yet.
    pub free: usize,
    pub errors: FormErrors,
    /// The GET form choosing the province, and the POST adding the wards.
    pub choose_action: String,
    pub action: String,
    pub max: usize,
}

#[derive(Serialize)]
pub struct BulkWardRow {
    /// The box's name (`ward.w28439`).
    pub field: String,
    /// `w28439`, for the box's id.
    pub value: String,
    pub label: String,
    pub preset: String,
    pub taken: bool,
    pub ticked: bool,
}

/// What the "add several" sheet shows: the province asked for, the wards
/// ticked when it came back refused, and why.
#[derive(Default)]
struct BulkView {
    open: bool,
    province: String,
    ticked: HashSet<String>,
    errors: FormErrors,
}

async fn bulk_sheet(state: &AppState, actor: &Actor, view: BulkView) -> Result<BulkSheet, Error> {
    let province = view.province.trim();
    let found = if province.is_empty() {
        None
    } else {
        state
            .usecases
            .admin
            .location_bulk_choices(actor, province)
            .await?
    };
    let (province, province_name, wards) = match found {
        Some(group) => (
            group.value,
            group.label,
            group
                .wards
                .into_iter()
                .map(|w| BulkWardRow {
                    field: format!("{WARD_PREFIX}{}", w.value),
                    ticked: !w.taken && view.ticked.contains(&w.value),
                    value: w.value,
                    label: w.label,
                    preset: w.preset,
                    taken: w.taken,
                })
                .collect(),
        ),
        None => (String::new(), String::new(), Vec::new()),
    };
    Ok(BulkSheet {
        open: view.open || !province.is_empty(),
        free: wards.iter().filter(|w: &&BulkWardRow| !w.taken).count(),
        province,
        province_name,
        wards,
        errors: view.errors,
        choose_action: url("admin_locations", &[])?,
        action: url("admin_locations_bulk", &[])?,
        max: LOCATION_BULK_MAX,
    })
}

/// The list page around the create form; `asked`: the row whose hide or
/// delete confirmation or edit sheet is open (`typed` after a refusal),
/// `refusal`: why a deletion was refused.
struct ListView {
    /// The list page shown (`?page=`).
    position: ListPosition,
    form: LocationForm,
    errors: FormErrors,
    asked: Asked,
    typed: Typed<LocationForm>,
    refusal: Option<String>,
    flash: Option<Flash>,
    status: StatusCode,
    bulk: BulkView,
}

async fn list_page(state: &AppState, actor: &Actor, view: ListView) -> Result<Response, Error> {
    let ListView {
        position,
        form,
        errors,
        asked,
        typed,
        refusal,
        flash,
        status,
        bulk,
    } = view;
    let position = position.checked(LISTING_LIST_STATES, CATALOGUE_SORTS);
    let criteria = position.filter(LISTING_LIST_STATES);
    let (locations, wards, bulk) = tokio::try_join!(
        state
            .usecases
            .admin
            .list_locations_page(actor, position.page(), &criteria),
        state.usecases.admin.location_choices(actor),
        bulk_sheet(state, actor, bulk),
    )?;
    let path = url("admin_locations", &[])?;
    let wanted_query = position.query();
    let wanted: Vec<(&str, &str)> = wanted_query
        .iter()
        .filter(|(k, _)| *k != "page")
        .map(|(k, v)| (*k, v.as_str()))
        .collect();
    if let Some(redirect) = past_the_end(&path, &wanted, locations.page, locations.total_pages) {
        return redirect;
    }
    let position = ListPosition {
        page: locations.page.to_string(),
        ..position
    };
    let edit = match asked.edit {
        Some(id) => {
            let (location, history) = tokio::try_join!(
                state.usecases.admin.get_location_admin(actor, id),
                history_rows(state, actor, HistoryOf::Location(id)),
            )?;
            let edited = EditedLocation {
                map_href: format!("{MAP_POINT_URL}{},{}", location.lat, location.lng),
                location,
            };
            Some(
                EditSheet::new(
                    edited,
                    typed,
                    |e: &EditedLocation| LocationForm::from_dto(&e.location),
                    position.record_href("admin_location", id)?,
                )
                .with_history(history),
            )
        }
        None => None,
    };
    let mut confirm = None;
    let mut rows = Vec::with_capacity(locations.items.len());
    for location in locations.items {
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
            edit_href: position.edit_href("admin_locations", &[], location.id)?,
            status_action,
            hide_href: position.href("admin_locations", &[("hide", &id)])?,
            delete_href: position.href("admin_locations", &[("delete", &id)])?,
            location,
        });
    }
    let search = position.search_box("admin_locations")?.sorted(
        state.locale(),
        &position,
        "admin_sort.display_order",
        CATALOGUE_SORTS,
    );
    let states = position.shared_state_chips(
        state.locale(),
        "admin_locations",
        LISTING_LIST_STATES,
        None,
    )?;
    let body = LocationsPage {
        search,
        states,
        total: locations.total,
        pagination: position.pagination(&path, locations.page, locations.total_pages),
        label_max: LOCATION_LABEL_MAX,
        rows,
        form,
        errors,
        create_action: path.clone(),
        confirm,
        edit,
        refusal,
        keep: ListReturn::new(position),
        flash,
        wards,
        bulk,
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
    /// The location whose edit sheet is open.
    pub edit: String,
    pub page: String,
    /// The words searched, shown / hidden.
    pub q: String,
    pub state: String,
    pub sort: String,
    /// The province whose wards the "add several" sheet offers (`p82`).
    pub bulk: String,
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
        position: ListPosition {
            page: q.page,
            q: q.q,
            state: q.state,
            sort: q.sort,
            ..ListPosition::default()
        },
        form: LocationForm::default(),
        errors: FormErrors::default(),
        asked: Asked::new(&q.hide, &q.delete).editing(&q.edit),
        typed: None,
        refusal: None,
        flash: flash.0,
        status: StatusCode::OK,
        bulk: BulkView {
            province: q.bulk,
            ..BulkView::default()
        },
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
        Ok(_) => ListPosition::default().back_to("admin_locations", Flash::LocationCreated),
        Err(err) if is_refusal(&err) => {
            let view = ListView {
                position: ListPosition::default(),
                errors: FormErrors::from_error(&state, &err),
                form,
                asked: Asked::default(),
                typed: None,
                refusal: None,
                flash: None,
                status: err.kind().status(),
                bulk: BulkView::default(),
            };
            list_page(&state, &actor, view).await
        }
        Err(err) => Err(err),
    }
}

/// POST /admin/locations/bulk: the ticked wards of the "add several" sheet
/// (`ward.w28439` boxes) and its `province`, to show it again when refused.
pub async fn create_from_wards(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(raw): ApiForm<HashMap<String, String>>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let wards: Vec<String> = raw
        .keys()
        .filter_map(|k| k.strip_prefix(WARD_PREFIX))
        .map(str::to_owned)
        .collect();
    let result = state
        .usecases
        .admin
        .create_locations_from_wards(&actor, &wards)
        .await;
    match result {
        Ok(done) => {
            let flash = if done.skipped.is_empty() {
                Flash::LocationsCreated
            } else {
                Flash::LocationsCreatedSomeExisted
            };
            let newest = ListPosition {
                sort: AdminSort::Newest.code().to_owned(),
                ..ListPosition::default()
            };
            newest.back_to("admin_locations", flash)
        }
        Err(err) if is_refusal(&err) => {
            let view = ListView {
                position: ListPosition::default(),
                form: LocationForm::default(),
                errors: FormErrors::default(),
                asked: Asked::default(),
                typed: None,
                refusal: None,
                flash: None,
                status: err.kind().status(),
                bulk: BulkView {
                    open: true,
                    province: raw.get("province").cloned().unwrap_or_default(),
                    ticked: wards.into_iter().collect(),
                    errors: FormErrors::from_error(&state, &err),
                },
            };
            list_page(&state, &actor, view).await
        }
        Err(err) => Err(err),
    }
}

/// GET /admin/locations/{id}: the preset's edit sheet on the list.
pub async fn show(ApiPath(raw_id): ApiPath<String>) -> Result<Response, Error> {
    let id = parse_id(&raw_id, ErrorCode::LocationNotFound)?;
    moved_permanently(&ListPosition::default().edit_href("admin_locations", &[], id)?)
}

/// POST /admin/locations/{id}: its edit sheet, back on the list position
/// of its query.
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiQuery(from): ApiQuery<ListPosition>,
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
        Ok(_) => from.back_to("admin_locations", Flash::LocationSaved),
        Err(err) if is_refusal(&err) => {
            let view = ListView {
                position: from,
                form: LocationForm::default(),
                errors: FormErrors::default(),
                asked: Asked {
                    edit: Some(id),
                    ..Asked::default()
                },
                typed: Some((form, FormErrors::from_error(&state, &err))),
                refusal: None,
                flash: None,
                status: err.kind().status(),
                bulk: BulkView::default(),
            };
            list_page(&state, &actor, view).await
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
    /// The list page the row was on (`ListReturn`).
    pub page: String,
    pub q: String,
    pub state: String,
    pub sort: String,
}

/// POST /admin/locations/{id}/status: back on the list.
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
    ListPosition {
        page: form.page,
        q: form.q,
        state: form.state,
        sort: form.sort,
        ..ListPosition::default()
    }
    .back_to("admin_locations", flash)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct DeleteForm {
    pub reason: String,
    /// The list page the row was on (`ListReturn`), when sent from the
    /// list; deleted, the list is shown there (or its new last page).
    pub page: String,
    pub q: String,
    pub state: String,
    pub sort: String,
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
    let position = ListPosition {
        page: form.page,
        q: form.q,
        state: form.state,
        sort: form.sort,
        ..ListPosition::default()
    };
    match result {
        Ok(()) => position.back_to("admin_locations", Flash::LocationDeleted),
        Err(err) if is_refusal(&err) => {
            let view = ListView {
                position,
                form: LocationForm::default(),
                errors: FormErrors::default(),
                asked: Asked::default(),
                typed: None,
                refusal: FormErrors::from_error(&state, &err).message,
                flash: None,
                status: err.kind().status(),
                bulk: BulkView::default(),
            };
            list_page(&state, &actor, view).await
        }
        Err(err) => Err(err),
    }
}
