//! /admin/alerts and /admin/alerts/{id}: what farmers must know at once (a
//! disease outbreak, salt water, a storm), for one sector or all, one
//! province or the whole region, between two days (`alert.manage`).
//!
//! The list carries the create sheet; an alert's page its edit form. Turning
//! one off asks for confirmation and a reason; turning it on is one button.
//! An alert is never deleted: it ends, or is turned off.

use application::{authz::Actor, inputs::AlertInput, outputs::AlertDto};
use axum::{extract::State, http::StatusCode, response::Response};
use chrono::NaiveDate;
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    labels::{alert_reach as reach, sector_options, CodeOption},
    middleware::auth::Viewer,
    pages::{
        admin::{
            alert_kinds::{self, TAB_KINDS},
            coded, is_refusal, render_admin, url, AdminTab, Asked, ListPosition, ListReturn,
            Outcome, RowConfirm, BACK_TO_LIST,
        },
        forms::{reason, FormErrors},
        parse_id, see_other,
    },
};

/// The form as typed: dates as the date boxes send them (`YYYY-MM-DD`).
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AlertForm {
    pub kind: String,
    pub sector: String,
    pub province_code: String,
    pub title: String,
    pub body: String,
    pub starts_on: String,
    pub ends_on: String,
}

fn date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").ok()
}

impl AlertForm {
    fn input(&self) -> AlertInput {
        AlertInput {
            kind: self.kind.clone(),
            sector: self.sector.clone(),
            province_code: self.province_code.clone(),
            title: self.title.clone(),
            body: self.body.clone(),
            starts_on: date(&self.starts_on),
            ends_on: date(&self.ends_on),
        }
    }

    fn from_dto(a: &AlertDto) -> Self {
        Self {
            kind: a.kind.clone(),
            sector: a.sector.clone().unwrap_or_default(),
            province_code: a.province_code.clone().unwrap_or_default(),
            title: a.title.clone(),
            body: a.body.clone(),
            starts_on: a.starts_on.format("%Y-%m-%d").to_string(),
            ends_on: a.ends_on.format("%Y-%m-%d").to_string(),
        }
    }
}

/// The choices of the form's selects.
#[derive(Serialize)]
pub struct AlertChoices {
    pub kinds: Vec<CodeOption>,
    pub sectors: Vec<CodeOption>,
    pub provinces: Vec<ProvinceOption>,
}

#[derive(Serialize)]
pub struct ProvinceOption {
    pub value: String,
    pub label: String,
}

async fn choices(state: &AppState, actor: &Actor) -> Result<AlertChoices, Error> {
    Ok(AlertChoices {
        kinds: state
            .usecases
            .alert_kind
            .named()
            .await?
            .into_iter()
            .map(|k| CodeOption {
                icon: k.icon.unwrap_or_default(),
                value: k.code,
                label: k.name,
            })
            .collect(),
        sectors: sector_options(&state.usecases.sector.named().await?),
        provinces: state
            .usecases
            .alert
            .province_choices(actor)
            .await?
            .into_iter()
            .map(|p| ProvinceOption {
                value: p.code,
                label: p.name,
            })
            .collect(),
    })
}

#[derive(Serialize)]
pub struct AlertRow {
    pub alert: AlertDto,
    pub kind_label: String,
    pub reach: String,
    pub href: String,
    pub status_action: String,
    pub hide_href: String,
}

#[derive(Serialize)]
pub struct AlertsPage {
    pub tabs: Vec<AdminTab>,
    pub rows: Vec<AlertRow>,
    pub choices: AlertChoices,
    pub form: AlertForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub confirm: Option<RowConfirm>,
    pub keep: ListReturn,
    pub flash: Option<Flash>,
}

struct ListView {
    form: AlertForm,
    errors: FormErrors,
    asked: Asked,
    flash: Option<Flash>,
    status: StatusCode,
}

async fn list_page(state: &AppState, actor: &Actor, view: ListView) -> Result<Response, Error> {
    let (alerts, sectors) = tokio::try_join!(
        state.usecases.alert.list(actor),
        state.usecases.sector.named(),
    )?;
    let position = ListPosition::default();
    let mut confirm = None;
    let mut rows = Vec::with_capacity(alerts.len());
    for alert in alerts {
        let id = alert.id.to_string();
        let status_action = url("admin_alert_status", &[("id", &id)])?;
        if view.asked.hide == Some(alert.id) && alert.is_active {
            confirm = Some(RowConfirm {
                kind: "hide",
                name: alert.title.clone(),
                action: status_action.clone(),
            });
        }
        rows.push(AlertRow {
            kind_label: alert.kind_name.clone(),
            reach: reach(state.locale(), &sectors, &alert),
            href: url("admin_alert", &[("id", &id)])?,
            status_action,
            hide_href: position.href("admin_alerts", &[("hide", &id)])?,
            alert,
        });
    }
    let path = url("admin_alerts", &[])?;
    let body = AlertsPage {
        tabs: tabs(state, false)?,
        rows,
        choices: choices(state, actor).await?,
        form: view.form,
        errors: view.errors,
        create_action: path.clone(),
        confirm,
        keep: ListReturn::new(position),
        flash: view.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        view.status,
        "pages/admin/alerts.html",
        &body,
    )
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    /// The alert whose "turn off" confirmation is open.
    pub hide: String,
    /// `kinds`: the second tab, the kinds of alert.
    pub tab: String,
}

/// The page's two tabs: the alerts, and their kinds (`kinds_tab`).
pub fn tabs(state: &AppState, kinds_tab: bool) -> Result<Vec<AdminTab>, Error> {
    Ok(vec![
        AdminTab {
            label: state.locale().text("alert.tab_alerts"),
            href: url("admin_alerts", &[])?,
            count: None,
            current: !kinds_tab,
        },
        AdminTab {
            label: state.locale().text("alert.tab_kinds"),
            href: ListPosition {
                tab: TAB_KINDS.to_owned(),
                ..ListPosition::default()
            }
            .href("admin_alerts", &[])?,
            count: None,
            current: kinds_tab,
        },
    ])
}

/// GET /admin/alerts
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    if q.tab == TAB_KINDS {
        let view = coded::ListView::asked(&q.hide, flash.0);
        let mut response = coded::list_page(&alert_kinds::PAGES, &state, &actor, view).await?;
        flash.clear_on(&mut response);
        return Ok(response);
    }
    let view = ListView {
        form: AlertForm::default(),
        errors: FormErrors::default(),
        asked: Asked::new(&q.hide, ""),
        flash: flash.0,
        status: StatusCode::OK,
    };
    let mut response = list_page(&state, &actor, view).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/alerts
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<AlertForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    match state.usecases.alert.create(&actor, form.input()).await {
        Ok(id) => redirect_to(id, Flash::AlertCreated),
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
pub struct AlertPage {
    pub alert: AlertDto,
    pub reach: String,
    pub choices: AlertChoices,
    pub form: AlertForm,
    pub errors: FormErrors,
    pub update_action: String,
    pub status_action: String,
    pub list_href: String,
    pub confirm_open: bool,
    pub flash: Option<Flash>,
}

async fn alert_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<AlertForm>,
) -> Result<Response, Error> {
    let (alert, sectors) = tokio::try_join!(
        state.usecases.alert.get(actor, id),
        state.usecases.sector.named(),
    )?;
    let id_text = id.to_string();
    let path = url("admin_alert", &[("id", &id_text)])?;
    let body = AlertPage {
        form: outcome.form.unwrap_or_else(|| AlertForm::from_dto(&alert)),
        reach: reach(state.locale(), &sectors, &alert),
        alert,
        choices: choices(state, actor).await?,
        errors: outcome.errors,
        update_action: path.clone(),
        status_action: url("admin_alert_status", &[("id", &id_text)])?,
        list_href: url("admin_alerts", &[])?,
        confirm_open: outcome.confirm_open,
        flash: outcome.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/alert.html",
        &body,
    )
}

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_alert", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// GET /admin/alerts/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::AlertNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let mut response = alert_page(&state, &actor, id, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/alerts/{id}
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<AlertForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::AlertNotFound)?;
    match state.usecases.alert.update(&actor, id, form.input()).await {
        Ok(()) => redirect_to(id, Flash::AlertSaved),
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            alert_page(&state, &actor, id, outcome).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    /// "true" to turn on, "false" to turn off.
    pub is_active: String,
    pub reason: String,
    /// `list` when sent from the list's row: land back there.
    pub back: String,
}

/// POST /admin/alerts/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::AlertNotFound)?;
    let on = form.is_active == "true";
    state
        .usecases
        .alert
        .set_active(&actor, id, on, reason(&form.reason))
        .await?;
    let flash = if on {
        Flash::AlertShown
    } else {
        Flash::AlertHidden
    };
    if form.back == BACK_TO_LIST {
        return ListPosition::default().back_to("admin_alerts", flash);
    }
    redirect_to(id, flash)
}
