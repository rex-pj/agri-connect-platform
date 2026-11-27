//! /admin/prices: the price board's admin page, two tabs. The first lists
//! every crop price entered (or one crop's, `?crop_id=`), the newest day
//! first, paged, each row linking to its crop and its area, with the "enter a
//! price" sheet (crop, area, day, low and high, a note on
//! where it was heard). Entering the same crop, area and day again replaces
//! that day's price, so a correction is just entering it again. Deleting
//! asks for confirmation and a reason: the row's link reloads the list with
//! `?delete={id}`, which renders that confirmation open. The second
//! (`?tab=crops`) is the produce the board follows (`crops::list_page`).

use application::{
    authz::Actor,
    constants::PRICE_FRESH_DAYS_DEFAULT,
    inputs::PriceInput,
    outputs::{AdminPagedResponse, CropPriceDto, NamedDto},
    usecases::demand_usecase::vn_today,
};
use axum::{extract::State, http::StatusCode, response::Response};
use chrono::{NaiveDate, Utc};
use domain::{models::price::PriceFilter, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{
            crops, is_refusal, render_admin, url, AdminTab, Asked, ListPosition, ProviderFilter,
            RowConfirm, SearchBox,
        },
        forms::{reason, vnd_amount, FormErrors},
        listing::{parse_opt, Pagination},
        parse_id, see_other,
    },
};

/// The form as typed: every box a string, so a bad number comes back as it
/// was entered.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct PriceForm {
    pub crop_id: String,
    pub location_id: String,
    pub observed_on: String,
    pub price_low: String,
    pub price_high: String,
    pub note: String,
}

impl PriceForm {
    /// A fresh form: today's date.
    fn today() -> Result<Self, Error> {
        Ok(Self {
            observed_on: vn_today(Utc::now())?.to_string(),
            ..Self::default()
        })
    }

    /// A fresh form for `crop` (the crop page's "enter today's price").
    fn today_for(crop: Option<i32>) -> Result<Self, Error> {
        Ok(Self {
            crop_id: crop.map(|c| c.to_string()).unwrap_or_default(),
            ..Self::today()?
        })
    }

    /// What the use case takes. A price box that is not a number is a field
    /// error now; a high box left blank means one price (high = low).
    fn input(&self) -> Result<PriceInput, Error> {
        let low = vnd_amount(&self.price_low, "price_low")?;
        let high = vnd_amount(&self.price_high, "price_high")?.or(low);
        Ok(PriceInput {
            crop_id: parse_opt(&self.crop_id),
            location_id: parse_opt(&self.location_id),
            observed_on: NaiveDate::parse_from_str(self.observed_on.trim(), "%Y-%m-%d").ok(),
            price_low: low,
            price_high: high,
            note: Some(self.note.clone()),
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    /// The price whose delete confirmation is open.
    pub delete: String,
    /// Only this crop's prices.
    pub crop_id: String,
    /// Only this area's prices.
    pub location_id: String,
    /// `crops`: the second tab, the produce followed.
    pub tab: String,
    /// The crop whose hide confirmation is open (crops tab).
    pub hide: String,
    /// The words searched (crops tab).
    pub q: String,
    /// One commodity, shown / hidden (crops tab).
    pub commodity_id: String,
    pub state: String,
    pub sort: String,
    /// The days observed between (prices tab).
    pub from: String,
    pub to: String,
    /// `stale`: only the prices past their freshness window (prices tab).
    pub gap: String,
}

/// The value of `?tab=` for the produce followed.
pub const TAB_CROPS: &str = "crops";

/// The page's two tabs, `crops_tab` telling which is shown.
pub fn tabs(state: &AppState, crops_tab: bool) -> Result<Vec<AdminTab>, Error> {
    let path = url("admin_prices", &[])?;
    Ok(vec![
        AdminTab {
            label: state.locale().text("admin_prices.tab_prices"),
            href: path.clone(),
            count: None,
            current: !crops_tab,
        },
        AdminTab {
            label: state.locale().text("admin_prices.tab_crops"),
            href: ListPosition {
                tab: TAB_CROPS.to_owned(),
                ..ListPosition::default()
            }
            .href("admin_prices", &[])?,
            count: None,
            current: crops_tab,
        },
    ])
}

#[derive(Serialize)]
pub struct PriceRow {
    pub price: CropPriceDto,
    /// The crop's and the area's admin pages.
    pub crop_href: String,
    pub area_href: String,
    pub delete_href: String,
}

#[derive(Serialize)]
pub struct PricesPage {
    pub tabs: Vec<AdminTab>,
    /// The crop the list is narrowed to, and the way back to every price.
    pub filter: Option<ProviderFilter>,
    /// The area select.
    pub search: SearchBox,
    pub total: i64,
    pub rows: Vec<PriceRow>,
    pub pagination: Option<Pagination>,
    pub crops: Vec<NamedDto>,
    pub areas: Vec<NamedDto>,
    pub form: PriceForm,
    pub errors: FormErrors,
    pub create_action: String,
    /// How long a new commodity's prices stay public; each commodity says
    /// its own (`price_fresh_days`).
    pub fresh_days: i64,
    pub confirm: Option<RowConfirm>,
    /// The hidden fields of the delete form: land back on this page.
    pub keep: ListPosition,
    pub flash: Option<Flash>,
}

/// The list around the sheet; `asked`: the row whose delete confirmation is
/// open.
struct ListView {
    position: ListPosition,
    form: PriceForm,
    errors: FormErrors,
    asked: Asked,
    flash: Option<Flash>,
    status: StatusCode,
}

async fn list_page(state: &AppState, actor: &Actor, view: ListView) -> Result<Response, Error> {
    let prices = &state.usecases.price;
    let criteria = PriceFilter {
        crop_id: crop_filter(&view.position),
        location_id: view.position.location_id(),
        observed_from: view.position.created_from(),
        observed_to: view.position.created_to(),
        stale: view.position.gap() == Some("stale"),
        ..PriceFilter::default()
    };
    let (listed, choices) = tokio::try_join!(
        prices.list_prices_in(
            actor,
            view.position.page(),
            &criteria,
            view.position.search()
        ),
        prices.price_choices(actor),
    )?;
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = listed;
    let position = ListPosition {
        page: page.to_string(),
        ..view.position
    };
    let mut confirm = None;
    let mut rows = Vec::with_capacity(items.len());
    for price in items {
        let id = price.id.to_string();
        if view.asked.delete == Some(price.id) {
            confirm = Some(RowConfirm {
                kind: "delete",
                name: format!("{} · {}", price.crop_name, price.location_label),
                action: url("admin_price_delete", &[("id", &id)])?,
            });
        }
        rows.push(PriceRow {
            crop_href: url("admin_crop", &[("id", &price.crop_id.to_string())])?,
            area_href: ListPosition::default().edit_href(
                "admin_locations",
                &[],
                price.location_id,
            )?,
            delete_href: position.href("admin_prices", &[("delete", &id)])?,
            price,
        });
    }
    let path = url("admin_prices", &[])?;
    let filter = match crop_filter(&position) {
        Some(id) => Some(ProviderFilter {
            icon: "fa-seedling",
            name: choices
                .crops
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.name.clone()),
            href: url("admin_crop", &[("id", &id.to_string())])?,
            clear_href: path.clone(),
        }),
        None => None,
    };
    let search = position
        .search_box("admin_prices")?
        .dated(state.locale())
        .toggled(
            "gap",
            "stale",
            state.locale().text("admin_toggle.stale_price"),
        )
        .picked(
            "location_id",
            state.locale().text("admin_pick.area"),
            state.locale().text("admin_pick.all_areas"),
            choices
                .areas
                .iter()
                .map(|a| (a.id, a.name.clone()))
                .collect(),
        );
    let body = PricesPage {
        tabs: tabs(state, false)?,
        filter,
        search,
        total,
        pagination: position.pagination(&path, page, total_pages),
        rows,
        crops: choices.crops,
        areas: choices.areas,
        form: view.form,
        errors: view.errors,
        create_action: path.clone(),
        fresh_days: i64::from(PRICE_FRESH_DAYS_DEFAULT),
        confirm,
        keep: position,
        flash: view.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        view.status,
        "pages/admin/prices.html",
        &body,
    )
}

/// The crop the list is narrowed to.
fn crop_filter(position: &ListPosition) -> Option<i32> {
    parse_opt::<i32>(&position.crop_id).filter(|c| *c > 0)
}

/// GET /admin/prices
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    if q.tab == TAB_CROPS {
        let mut response = crops::list_page(
            &state,
            &actor,
            crops::ListView::asked(&q.hide, flash.0, &state).narrowed(
                q.q,
                q.commodity_id,
                q.state,
                q.sort,
            ),
        )
        .await?;
        flash.clear_on(&mut response);
        return Ok(response);
    }
    let position = ListPosition {
        page: q.page,
        crop_id: q.crop_id,
        location_id: q.location_id,
        q: q.q,
        from: q.from,
        to: q.to,
        gap: q.gap,
        ..ListPosition::default()
    };
    let view = ListView {
        form: PriceForm::today_for(crop_filter(&position))?,
        position,
        errors: FormErrors::default(),
        asked: Asked::new("", &q.delete),
        flash: flash.0,
        status: StatusCode::OK,
    };
    let mut response = list_page(&state, &actor, view).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/prices: back to the first page (where the newest day is),
/// or the page again with the sheet open and its messages.
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<PriceForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let result = async {
        let input = form.input()?;
        state.usecases.price.record_price(&actor, input).await
    }
    .await;
    match result {
        Ok(_) => ListPosition::default().back_to("admin_prices", Flash::PriceRecorded),
        Err(err) if is_refusal(&err) => {
            let view = ListView {
                position: ListPosition::default(),
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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct DeleteForm {
    pub reason: String,
    /// The list page to land back on, and its crop filter.
    pub page: String,
    pub crop_id: String,
    pub location_id: String,
    pub q: String,
    pub from: String,
    pub to: String,
    pub gap: String,
}

/// POST /admin/prices/{id}/delete
pub async fn delete(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<DeleteForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PriceNotFound)?;
    state
        .usecases
        .price
        .delete_price(&actor, id, reason(&form.reason))
        .await?;
    ListPosition {
        page: form.page,
        crop_id: form.crop_id,
        location_id: form.location_id,
        q: form.q,
        from: form.from,
        to: form.to,
        gap: form.gap,
        ..ListPosition::default()
    }
    .back_to("admin_prices", Flash::PriceDeleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_high_box_means_one_price() {
        let form = PriceForm {
            crop_id: "3".into(),
            location_id: "1".into(),
            observed_on: "2026-11-07".into(),
            price_low: "6.200".into(),
            price_high: " ".into(),
            note: String::new(),
        };
        let input = form.input().ok();
        assert_eq!(input.as_ref().and_then(|i| i.price_low), Some(6_200));
        assert_eq!(input.as_ref().and_then(|i| i.price_high), Some(6_200));
        assert_eq!(input.as_ref().and_then(|i| i.crop_id), Some(3));
        let bad = PriceForm {
            price_low: "sáu nghìn".into(),
            ..PriceForm::default()
        };
        assert!(bad.input().is_err());
    }
}
