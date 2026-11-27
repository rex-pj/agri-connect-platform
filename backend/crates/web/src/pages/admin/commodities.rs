//! /admin/commodities and /admin/commodities/{id}: what farmers raise (cây /
//! con: rice, durian, pigs, shrimp), each in one sector, with a create
//! sheet and an edit sheet (`EditSheet`, `?edit={id}`), and one
//! commodity's page: what hangs on it (its crops, its supplies, the board),
//! its edit form and visibility. The price board
//! groups its items by them and a supply may say which one it is for. A
//! commodity is never deleted (its crops and supplies point to it): it is
//! hidden.
//!
//! Hiding asks for confirmation and a reason (a dialog with JS, the same
//! form on the page without); showing again is one button. The list's rows
//! carry both (`?hide={id}` opens the confirmation there); sent with
//! `back=list`, they land back on the list.

use application::catalogue::PERM_LISTING_MODERATE;
use application::{
    authz::Actor,
    constants::PRICE_FRESH_DAYS_DEFAULT,
    inputs::CommodityInput,
    outputs::{CommodityDto, CropDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::models::{admin_list::LISTING_LIST_STATES, product::AdminProductFilter};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    labels::{sector_name, sector_options, CodeOption},
    middleware::auth::Viewer,
    pages::{
        admin::{
            catalogue_tabs, crops::CropForm, is_refusal, render_admin, url, AdminTab, Asked,
            CatalogueTab, EditSheet, ListPosition, ListReturn, Outcome, RowConfirm, SearchBox,
            Typed, BACK_TO_LIST, CATALOGUE_SORTS,
        },
        forms::{reason, whole_number, FormErrors},
        listing::{past_the_end, with_query, Pagination},
        parse_id, see_other,
    },
};

/// The form as typed.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct CommodityForm {
    pub name: String,
    /// A sector code.
    pub sector: String,
    pub icon: String,
    pub display_order: String,
    /// Days a price stays on the board; blank is the default.
    pub price_fresh_days: String,
}

impl CommodityForm {
    fn input(&self) -> Result<CommodityInput, Error> {
        Ok(CommodityInput {
            name: self.name.clone(),
            sector: self.sector.clone(),
            icon: Some(self.icon.clone()),
            display_order: whole_number(&self.display_order, "display_order")?,
            price_fresh_days: match self.price_fresh_days.trim() {
                "" => PRICE_FRESH_DAYS_DEFAULT,
                raw => whole_number(raw, "price_fresh_days")?,
            },
        })
    }

    fn from_dto(c: &CommodityDto) -> Self {
        Self {
            name: c.name.clone(),
            sector: c.sector.clone(),
            icon: c.icon.clone().unwrap_or_default(),
            display_order: c.display_order.to_string(),
            price_fresh_days: c.price_fresh_days.to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct CommodityRow {
    pub commodity: CommodityDto,
    pub sector_label: String,
    pub href: String,
    /// The list again with its edit sheet open.
    pub edit_href: String,
    pub status_action: String,
    pub hide_href: String,
}

#[derive(Serialize)]
pub struct CommoditiesPage {
    /// The classification screen's tabs, this list's marked.
    pub tabs: Vec<AdminTab>,
    pub rows: Vec<CommodityRow>,
    pub sectors: Vec<CodeOption>,
    pub form: CommodityForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub confirm: Option<RowConfirm>,
    /// The commodity whose edit sheet is open.
    pub edit: Option<EditSheet<CommodityDto, CommodityForm>>,
    pub search: SearchBox,
    pub keep: ListReturn,
    /// Every commodity, for the header's count.
    pub total: i64,
    /// The shown / hidden chips.
    pub states: Vec<AdminTab>,
    pub pagination: Option<Pagination>,
    pub flash: Option<Flash>,
}

struct ListView {
    /// The list page shown (`?page=`).
    position: ListPosition,
    form: CommodityForm,
    errors: FormErrors,
    asked: Asked,
    /// The edit as typed after a refusal.
    typed: Typed<CommodityForm>,
    flash: Option<Flash>,
    status: StatusCode,
}

async fn list_page(state: &AppState, actor: &Actor, view: ListView) -> Result<Response, Error> {
    let asked_position = view
        .position
        .clone()
        .checked(LISTING_LIST_STATES, CATALOGUE_SORTS);
    let criteria = asked_position.filter(LISTING_LIST_STATES);
    let (commodities, sectors) = tokio::try_join!(
        state
            .usecases
            .commodity
            .list_page(actor, asked_position.page(), &criteria,),
        state.usecases.sector.named(),
    )?;
    let path = url("admin_commodities", &[])?;
    let mut wanted_query = asked_position.query();
    wanted_query.retain(|(k, _)| *k != "page");
    let wanted: Vec<(&str, &str)> = wanted_query.iter().map(|(k, v)| (*k, v.as_str())).collect();
    if let Some(redirect) = past_the_end(&path, &wanted, commodities.page, commodities.total_pages)
    {
        return redirect;
    }
    let position = ListPosition {
        page: commodities.page.to_string(),
        ..asked_position
    };
    let edit = match view.asked.edit {
        Some(id) => {
            let commodity = state.usecases.commodity.get(actor, id).await?;
            Some(EditSheet::new(
                commodity,
                view.typed,
                CommodityForm::from_dto,
                sheet_action(&position, id)?,
            ))
        }
        None => None,
    };
    let mut confirm = None;
    let mut rows = Vec::with_capacity(commodities.items.len());
    for commodity in commodities.items {
        let id = commodity.id.to_string();
        let status_action = url("admin_commodity_status", &[("id", &id)])?;
        if view.asked.hide == Some(commodity.id) && commodity.is_active {
            confirm = Some(RowConfirm {
                kind: "hide",
                name: commodity.name.clone(),
                action: status_action.clone(),
            });
        }
        rows.push(CommodityRow {
            sector_label: sector_name(&sectors, &commodity.sector),
            href: url("admin_commodity", &[("id", &id)])?,
            edit_href: position.edit_href("admin_commodities", &[], commodity.id)?,
            status_action,
            hide_href: position.href("admin_commodities", &[("hide", &id)])?,
            commodity,
        });
    }
    let locale = state.locale();
    let search = position
        .search_box("admin_commodities")?
        .picked(
            "sector",
            locale.text("sector.label"),
            locale.text("admin_pick.all_sectors"),
            sectors
                .iter()
                .map(|s| (s.code.clone(), s.name.clone()))
                .collect(),
        )
        .sorted(
            locale,
            &position,
            "admin_sort.display_order",
            CATALOGUE_SORTS,
        );
    let states =
        position.shared_state_chips(locale, "admin_commodities", LISTING_LIST_STATES, None)?;
    let body = CommoditiesPage {
        search,
        total: commodities.total,
        states,
        pagination: position.pagination(&path, commodities.page, commodities.total_pages),
        tabs: catalogue_tabs(state, CatalogueTab::Commodities)?,
        rows,
        sectors: sector_options(&sectors),
        form: view.form,
        errors: view.errors,
        create_action: path.clone(),
        confirm,
        edit,
        keep: ListReturn::new(position),
        flash: view.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        view.status,
        "pages/admin/commodities.html",
        &body,
    )
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    /// The commodity whose hide confirmation is open.
    pub hide: String,
    /// The commodity whose edit sheet is open.
    pub edit: String,
    pub page: String,
    /// The words searched, the sector, shown / hidden.
    pub q: String,
    pub sector: String,
    pub state: String,
    pub sort: String,
}

/// GET /admin/commodities
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
            sector: q.sector,
            state: q.state,
            sort: q.sort,
            ..ListPosition::default()
        },
        form: CommodityForm {
            price_fresh_days: PRICE_FRESH_DAYS_DEFAULT.to_string(),
            ..CommodityForm::default()
        },
        errors: FormErrors::default(),
        asked: Asked::new(&q.hide, "").editing(&q.edit),
        typed: None,
        flash: flash.0,
        status: StatusCode::OK,
    };
    let mut response = list_page(&state, &actor, view).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/commodities
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<CommodityForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let result = async {
        let input = form.input()?;
        state.usecases.commodity.create(&actor, input).await
    }
    .await;
    match result {
        Ok(_) => ListPosition::default().back_to("admin_commodities", Flash::CommodityCreated),
        Err(err) if is_refusal(&err) => {
            let view = ListView {
                position: ListPosition::default(),
                errors: FormErrors::from_error(&state, &err),
                form,
                asked: Asked::default(),
                typed: None,
                flash: None,
                status: err.kind().status(),
            };
            list_page(&state, &actor, view).await
        }
        Err(err) => Err(err),
    }
}

/// A crop of the commodity, linked to its page.
#[derive(Serialize)]
pub struct CropLink {
    pub crop: CropDto,
    pub href: String,
}

#[derive(Serialize)]
pub struct CommodityPage {
    pub commodity: CommodityDto,
    /// Its items on the price board (hidden ones too), each with its page.
    pub crops: Vec<CropLink>,
    /// The sheet that adds one, this commodity chosen.
    pub crop_form: CropForm,
    pub crop_commodities: Vec<CommodityDto>,
    pub crop_create_action: String,
    /// How many supplies are for it, and the admin list of them.
    pub supplies_total: Option<i64>,
    pub supplies_href: String,
    /// The public price board, where its section shows.
    pub board_href: String,
    pub sectors: Vec<CodeOption>,
    pub form: CommodityForm,
    pub errors: FormErrors,
    pub update_action: String,
    pub status_action: String,
    pub list_href: String,
    pub confirm_open: bool,
    pub flash: Option<Flash>,
}

async fn commodity_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<CommodityForm>,
) -> Result<Response, Error> {
    let prices = &state.usecases.price;
    let (commodity, crops, crop_commodities, sectors) = tokio::try_join!(
        state.usecases.commodity.get(actor, id),
        prices.crops_of(actor, id),
        prices.commodity_choices(actor),
        state.usecases.sector.named(),
    )?;
    // The supplies for it, when the visitor may moderate supplies.
    let supplies_total = if actor.can(PERM_LISTING_MODERATE) {
        let of_commodity = AdminProductFilter {
            commodity_id: Some(id),
            ..AdminProductFilter::default()
        };
        let listed = state
            .usecases
            .admin
            .list_products_admin(actor, 1, &of_commodity)
            .await?;
        Some(listed.total)
    } else {
        None
    };
    let id_text = id.to_string();
    let path = url("admin_commodity", &[("id", &id_text)])?;
    let crops = crops
        .into_iter()
        .map(|crop| {
            Ok(CropLink {
                href: url("admin_crop", &[("id", &crop.id.to_string())])?,
                crop,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let body = CommodityPage {
        crops,
        crop_form: CropForm::for_commodity(state, id),
        crop_commodities,
        crop_create_action: url("admin_crops", &[])?,
        supplies_total,
        supplies_href: ListPosition {
            commodity_id: id_text.clone(),
            ..ListPosition::default()
        }
        .href("admin_products", &[])?,
        board_href: url("prices", &[])?,
        form: outcome
            .form
            .unwrap_or_else(|| CommodityForm::from_dto(&commodity)),
        commodity,
        sectors: sector_options(&sectors),
        errors: outcome.errors,
        update_action: path.clone(),
        status_action: url("admin_commodity_status", &[("id", &id_text)])?,
        list_href: url("admin_commodities", &[])?,
        confirm_open: outcome.confirm_open,
        flash: outcome.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/commodity.html",
        &body,
    )
}

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_commodity", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// Where a commodity's edit sheet posts: its address, with the list
/// position to land back on (`back=list`).
fn sheet_action(position: &ListPosition, id: i32) -> Result<String, Error> {
    let query = position.query();
    let mut params = vec![("back", BACK_TO_LIST)];
    params.extend(query.iter().map(|(k, v)| (*k, v.as_str())));
    Ok(with_query(
        &url("admin_commodity", &[("id", &id.to_string())])?,
        &params,
    ))
}

/// The query of an edit posted from the list's sheet.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct BackQuery {
    pub back: String,
}

/// GET /admin/commodities/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::CommodityNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let mut response = commodity_page(&state, &actor, id, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/commodities/{id}: from its page, back there; from the
/// list's edit sheet (`back=list`), back on that list position.
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiQuery(back): ApiQuery<BackQuery>,
    ApiQuery(from): ApiQuery<ListPosition>,
    ApiForm(form): ApiForm<CommodityForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::CommodityNotFound)?;
    let result = async {
        let input = form.input()?;
        state.usecases.commodity.update(&actor, id, input).await
    }
    .await;
    match result {
        Ok(()) if back.back == BACK_TO_LIST => {
            from.back_to("admin_commodities", Flash::CommoditySaved)
        }
        Ok(()) => redirect_to(id, Flash::CommoditySaved),
        Err(err) if is_refusal(&err) && back.back == BACK_TO_LIST => {
            let view = ListView {
                position: from,
                form: CommodityForm::default(),
                errors: FormErrors::default(),
                asked: Asked {
                    edit: Some(id),
                    ..Asked::default()
                },
                typed: Some((form, FormErrors::from_error(&state, &err))),
                flash: None,
                status: err.kind().status(),
            };
            list_page(&state, &actor, view).await
        }
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            commodity_page(&state, &actor, id, outcome).await
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
    /// The list page the row was on (`ListReturn`).
    pub page: String,
    pub q: String,
    pub sector: String,
    pub state: String,
    pub sort: String,
}

/// POST /admin/commodities/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::CommodityNotFound)?;
    let show = form.is_active == "true";
    state
        .usecases
        .commodity
        .set_active(&actor, id, show, reason(&form.reason))
        .await?;
    let flash = if show {
        Flash::CommodityShown
    } else {
        Flash::CommodityHidden
    };
    if form.back == BACK_TO_LIST {
        let position = ListPosition {
            page: form.page,
            q: form.q,
            sector: form.sector,
            state: form.state,
            sort: form.sort,
            ..ListPosition::default()
        };
        return position.back_to("admin_commodities", flash);
    }
    redirect_to(id, flash)
}
