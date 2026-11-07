//! /admin/crops and /admin/crops/{id}: the crops the price page follows (a
//! rice variety, a fruit), with a create sheet, and one crop's edit form and
//! visibility. A crop is never deleted (its prices would go with it): it is
//! hidden, which takes it and its prices off the public page.
//!
//! Hiding asks for confirmation and a reason (a dialog with JS, the same
//! form on the page without); showing again is one button. The list's rows
//! carry both (`?hide={id}` opens the confirmation there); sent with
//! `back=list`, they land back on the list.

use application::{authz::Actor, inputs::CropInput, outputs::CropDto};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{
    constants::{CROP_KINDS, CROP_KIND_FRUIT, CROP_KIND_RICE},
    Error, ErrorCode,
};
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
        forms::{reason, whole_number, FormErrors},
        parse_id, see_other,
    },
};

/// The form as typed.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct CropForm {
    pub name: String,
    pub kind: String,
    pub unit: String,
    pub display_order: String,
}

impl CropForm {
    /// A fresh form: a fruit sold by the kilogram.
    fn fresh(state: &AppState) -> Self {
        Self {
            kind: CROP_KIND_FRUIT.to_owned(),
            unit: state.locale().text("crop_form.unit_default"),
            ..Self::default()
        }
    }

    fn input(&self) -> Result<CropInput, Error> {
        Ok(CropInput {
            name: self.name.clone(),
            kind: self.kind.clone(),
            unit: self.unit.clone(),
            display_order: whole_number(&self.display_order, "display_order")?,
        })
    }

    fn from_dto(c: &CropDto) -> Self {
        Self {
            name: c.name.clone(),
            kind: c.kind.clone(),
            unit: c.unit.clone(),
            display_order: c.display_order.to_string(),
        }
    }
}

/// A crop kind for the select: its code and its word.
#[derive(Serialize)]
pub struct KindOption {
    pub value: &'static str,
    pub label: String,
}

fn kind_label(state: &AppState, kind: &str) -> String {
    let key = if kind == CROP_KIND_RICE {
        "prices.rice"
    } else {
        "prices.fruit"
    };
    state.locale().text(key)
}

fn kind_options(state: &AppState) -> Vec<KindOption> {
    CROP_KINDS
        .iter()
        .map(|k| KindOption {
            value: k,
            label: kind_label(state, k),
        })
        .collect()
}

#[derive(Serialize)]
pub struct CropRow {
    pub crop: CropDto,
    pub kind_label: String,
    pub href: String,
    pub status_action: String,
    pub hide_href: String,
}

#[derive(Serialize)]
pub struct CropsPage {
    pub rows: Vec<CropRow>,
    pub kinds: Vec<KindOption>,
    pub form: CropForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub prices_href: String,
    pub confirm: Option<RowConfirm>,
    pub keep: ListReturn,
    pub flash: Option<Flash>,
}

struct ListView {
    form: CropForm,
    errors: FormErrors,
    asked: Asked,
    flash: Option<Flash>,
    status: StatusCode,
}

async fn list_page(state: &AppState, actor: &Actor, view: ListView) -> Result<Response, Error> {
    let crops = state.usecases.price.list_crops(actor).await?;
    let position = ListPosition::default();
    let mut confirm = None;
    let mut rows = Vec::with_capacity(crops.len());
    for crop in crops {
        let id = crop.id.to_string();
        let status_action = url("admin_crop_status", &[("id", &id)])?;
        if view.asked.hide == Some(crop.id) && crop.is_active {
            confirm = Some(RowConfirm {
                kind: "hide",
                name: crop.name.clone(),
                action: status_action.clone(),
            });
        }
        rows.push(CropRow {
            kind_label: kind_label(state, &crop.kind),
            href: url("admin_crop", &[("id", &id)])?,
            status_action,
            hide_href: position.href("admin_crops", &[("hide", &id)])?,
            crop,
        });
    }
    let path = url("admin_crops", &[])?;
    let body = CropsPage {
        rows,
        kinds: kind_options(state),
        form: view.form,
        errors: view.errors,
        create_action: path.clone(),
        prices_href: url("admin_prices", &[])?,
        confirm,
        keep: ListReturn::new(position),
        flash: view.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        view.status,
        "pages/admin/crops.html",
        &body,
    )
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    /// The crop whose hide confirmation is open.
    pub hide: String,
}

/// GET /admin/crops
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
        form: CropForm::fresh(&state),
        errors: FormErrors::default(),
        asked: Asked::new(&q.hide, ""),
        flash: flash.0,
        status: StatusCode::OK,
    };
    let mut response = list_page(&state, &actor, view).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/crops
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<CropForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let result = async {
        let input = form.input()?;
        state.usecases.price.create_crop(&actor, input).await
    }
    .await;
    match result {
        Ok(id) => redirect_to(id, Flash::CropCreated),
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
pub struct CropPage {
    pub crop: CropDto,
    pub kinds: Vec<KindOption>,
    pub form: CropForm,
    pub errors: FormErrors,
    pub update_action: String,
    pub status_action: String,
    pub list_href: String,
    pub confirm_open: bool,
    pub flash: Option<Flash>,
}

async fn crop_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<CropForm>,
) -> Result<Response, Error> {
    let crop = state.usecases.price.get_crop(actor, id).await?;
    let id_text = id.to_string();
    let path = url("admin_crop", &[("id", &id_text)])?;
    let body = CropPage {
        form: outcome.form.unwrap_or_else(|| CropForm::from_dto(&crop)),
        crop,
        kinds: kind_options(state),
        errors: outcome.errors,
        update_action: path.clone(),
        status_action: url("admin_crop_status", &[("id", &id_text)])?,
        list_href: url("admin_crops", &[])?,
        confirm_open: outcome.confirm_open,
        flash: outcome.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/crop.html",
        &body,
    )
}

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_crop", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// GET /admin/crops/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::CropNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let mut response = crop_page(&state, &actor, id, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/crops/{id}
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<CropForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::CropNotFound)?;
    let result = async {
        let input = form.input()?;
        state.usecases.price.update_crop(&actor, id, input).await
    }
    .await;
    match result {
        Ok(()) => redirect_to(id, Flash::CropSaved),
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            crop_page(&state, &actor, id, outcome).await
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

/// POST /admin/crops/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::CropNotFound)?;
    let show = form.is_active == "true";
    state
        .usecases
        .price
        .set_crop_active(&actor, id, show, reason(&form.reason))
        .await?;
    let flash = if show {
        Flash::CropShown
    } else {
        Flash::CropHidden
    };
    if form.back == BACK_TO_LIST {
        return ListPosition::default().back_to("admin_crops", flash);
    }
    redirect_to(id, flash)
}
