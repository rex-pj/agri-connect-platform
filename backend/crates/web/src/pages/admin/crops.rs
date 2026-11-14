//! The price board's second tab (`/admin/prices?tab=crops`, where the former
//! `/admin/crops` list moves for good) and /admin/crops/{id}: the farm
//! produce the price page
//! follows (a rice variety, a fruit grade, live pigs), each of one commodity
//! (cây trồng, vật nuôi, kept on /admin/commodities), with a create sheet, and one crop's edit form and
//! visibility. A crop is never deleted (its prices would go with it): it is
//! hidden, which takes it and its prices off the public page.
//!
//! Hiding asks for confirmation and a reason (a dialog with JS, the same
//! form on the page without); showing again is one button. The list's rows
//! carry both (`?hide={id}` opens the confirmation there); sent with
//! `back=list`, they land back on the list.

use application::{
    authz::Actor,
    inputs::CropInput,
    outputs::{CommodityDto, CropDto, CropPriceDto},
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
            is_refusal, prices, render_admin, url, AdminTab, Asked, ListPosition, ListReturn,
            Outcome, RowConfirm, BACK_TO_LIST,
        },
        forms::{reason, whole_number, FormErrors},
        listing::parse_opt,
        moved_permanently, parse_id, see_other,
    },
};

/// The form as typed.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct CropForm {
    pub name: String,
    /// The commodity's id, as sent.
    pub commodity_id: String,
    pub unit: String,
    pub display_order: String,
}

impl CropForm {
    /// A fresh form: no commodity chosen yet, sold by the kilogram.
    fn fresh(state: &AppState) -> Self {
        Self {
            unit: state.locale().text("crop_form.unit_default"),
            ..Self::default()
        }
    }

    /// A fresh form for a crop of `commodity` (its own page's sheet).
    pub fn for_commodity(state: &AppState, commodity: i32) -> Self {
        Self {
            commodity_id: commodity.to_string(),
            ..Self::fresh(state)
        }
    }

    fn input(&self) -> Result<CropInput, Error> {
        Ok(CropInput {
            name: self.name.clone(),
            commodity_id: parse_opt(&self.commodity_id),
            unit: self.unit.clone(),
            display_order: whole_number(&self.display_order, "display_order")?,
        })
    }

    fn from_dto(c: &CropDto) -> Self {
        Self {
            name: c.name.clone(),
            commodity_id: c.commodity_id.to_string(),
            unit: c.unit.clone(),
            display_order: c.display_order.to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct CropRow {
    pub crop: CropDto,
    pub href: String,
    pub status_action: String,
    pub hide_href: String,
}

#[derive(Serialize)]
pub struct CropsPage {
    pub tabs: Vec<AdminTab>,
    pub rows: Vec<CropRow>,
    /// The shown commodities the form offers.
    pub commodities: Vec<CommodityDto>,
    /// Where the commodities are kept.
    pub commodities_href: String,
    pub form: CropForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub confirm: Option<RowConfirm>,
    pub keep: ListReturn,
    pub flash: Option<Flash>,
}

/// The list around the create sheet.
pub struct ListView {
    form: CropForm,
    errors: FormErrors,
    asked: Asked,
    flash: Option<Flash>,
    status: StatusCode,
}

impl ListView {
    /// The list as first shown: a fresh form, maybe a row's hide
    /// confirmation open (`hide`), maybe a flash.
    pub fn asked(hide: &str, flash: Option<Flash>, state: &AppState) -> Self {
        Self {
            form: CropForm::fresh(state),
            errors: FormErrors::default(),
            asked: Asked::new(hide, ""),
            flash,
            status: StatusCode::OK,
        }
    }
}

/// Where the list lives: the price board's crops tab.
fn list_position() -> ListPosition {
    ListPosition {
        tab: prices::TAB_CROPS.to_owned(),
        ..ListPosition::default()
    }
}

pub async fn list_page(state: &AppState, actor: &Actor, view: ListView) -> Result<Response, Error> {
    let prices = &state.usecases.price;
    let (crops, commodities) =
        tokio::try_join!(prices.list_crops(actor), prices.commodity_choices(actor))?;
    let position = list_position();
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
            href: url("admin_crop", &[("id", &id)])?,
            status_action,
            hide_href: position.href("admin_prices", &[("hide", &id)])?,
            crop,
        });
    }
    let path = url("admin_prices", &[])?;
    let body = CropsPage {
        tabs: prices::tabs(state, true)?,
        rows,
        commodities,
        commodities_href: url("admin_commodities", &[])?,
        form: view.form,
        errors: view.errors,
        create_action: url("admin_crops", &[])?,
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

/// GET /admin/crops → /admin/prices?tab=crops (the list moved there for
/// good; a hide confirmation asked for stays asked).
pub async fn list(ApiQuery(q): ApiQuery<ListQuery>) -> Result<Response, Error> {
    let hide = parse_opt::<i32>(&q.hide).map(|id| id.to_string());
    let extra: Vec<(&str, &str)> = hide.iter().map(|h| ("hide", h.as_str())).collect();
    moved_permanently(&list_position().href("admin_prices", &extra)?)
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
    /// Its commodity's admin page.
    pub commodity_href: String,
    /// Its latest prices (one list page), every one of them, and the price
    /// sheet with it chosen.
    pub prices: Vec<CropPriceDto>,
    pub prices_total: i64,
    pub prices_href: String,
    pub enter_price_href: String,
    pub commodities: Vec<CommodityDto>,
    pub commodities_href: String,
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
    let prices = &state.usecases.price;
    let (crop, commodities, recent) = tokio::try_join!(
        prices.get_crop(actor, id),
        prices.commodity_choices(actor),
        prices.list_prices(actor, Some(id), 1),
    )?;
    let id_text = id.to_string();
    let of_crop = ListPosition {
        crop_id: id_text.clone(),
        ..ListPosition::default()
    };
    let path = url("admin_crop", &[("id", &id_text)])?;
    let body = CropPage {
        form: outcome.form.unwrap_or_else(|| CropForm::from_dto(&crop)),
        commodity_href: url("admin_commodity", &[("id", &crop.commodity_id.to_string())])?,
        prices: recent.items,
        prices_total: recent.total,
        prices_href: of_crop.href("admin_prices", &[])?,
        enter_price_href: format!("{}#nhap-gia", of_crop.href("admin_prices", &[])?),
        crop,
        commodities,
        commodities_href: url("admin_commodities", &[])?,
        errors: outcome.errors,
        update_action: path.clone(),
        status_action: url("admin_crop_status", &[("id", &id_text)])?,
        list_href: list_position().href("admin_prices", &[])?,
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
        return list_position().back_to("admin_prices", flash);
    }
    redirect_to(id, flash)
}
