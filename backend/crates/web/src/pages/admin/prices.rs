//! /admin/prices: every crop price entered, the newest day first, paged,
//! with the "enter a price" sheet (crop, area, day, low and high, a note on
//! where it was heard). Entering the same crop, area and day again replaces
//! that day's price, so a correction is just entering it again. Deleting
//! asks for confirmation and a reason: the row's link reloads the list with
//! `?delete={id}`, which renders that confirmation open.

use application::{
    authz::Actor,
    constants::PRICE_FRESH_DAYS,
    inputs::PriceInput,
    outputs::{AdminPagedResponse, CropPriceDto, NamedDto},
    usecases::demand_usecase::vn_today,
};
use axum::{extract::State, http::StatusCode, response::Response};
use chrono::{NaiveDate, Utc};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{is_refusal, render_admin, url, Asked, ListPosition, RowConfirm},
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
}

#[derive(Serialize)]
pub struct PriceRow {
    pub price: CropPriceDto,
    pub delete_href: String,
}

#[derive(Serialize)]
pub struct PricesPage {
    pub total: i64,
    pub rows: Vec<PriceRow>,
    pub pagination: Option<Pagination>,
    pub crops: Vec<NamedDto>,
    pub areas: Vec<NamedDto>,
    pub form: PriceForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub crops_href: String,
    /// How long a price stays public (`PRICE_FRESH_DAYS`).
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
    let (listed, choices) = tokio::try_join!(
        prices.list_prices(actor, view.position.page()),
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
            delete_href: position.href("admin_prices", &[("delete", &id)])?,
            price,
        });
    }
    let path = url("admin_prices", &[])?;
    let body = PricesPage {
        total,
        pagination: position.pagination(&path, page, total_pages),
        rows,
        crops: choices.crops,
        areas: choices.areas,
        form: view.form,
        errors: view.errors,
        create_action: path.clone(),
        crops_href: url("admin_crops", &[])?,
        fresh_days: PRICE_FRESH_DAYS,
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
    let view = ListView {
        position: ListPosition {
            page: q.page,
            ..ListPosition::default()
        },
        form: PriceForm::today()?,
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
    /// The list page to land back on.
    pub page: String,
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
