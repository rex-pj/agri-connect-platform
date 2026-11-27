//! The kinds of alert (dịch bệnh, xâm nhập mặn, thời tiết): the alerts
//! page's second tab (`/admin/alerts?tab=kinds`, `alerts::list` hands it
//! here), their create sheet (POST /admin/alert-kinds) and edit sheet (POST
//! /admin/alert-kinds/{id}; its GET opens the sheet on the tab).
//! `/admin/alert-kinds` 301s to the tab. The pages are the shared
//! coded-list pages (`coded`); this module says where and in which words.

use application::usecases::admin_usecase::HistoryOf;
use axum::{extract::State, response::Response};
use domain::{Error, ErrorCode};

use crate::{
    app_state::AppState,
    cookies::Flash,
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{
            alerts,
            coded::{self, CodedForm, CodedPages, CodedWordKeys, StatusForm},
            url, ListPosition,
        },
        moved_permanently, see_other,
    },
};

/// The alerts page's `?tab=` of the kinds.
pub const TAB_KINDS: &str = "kinds";

pub const PAGES: CodedPages = CodedPages {
    usecase: |state| &state.usecases.alert_kind,
    list_route: "admin_alerts",
    list_tab: TAB_KINDS,
    tabs: Some(alerts::tabs),
    create_route: "admin_alert_kinds",
    entry_route: "admin_alert_kind",
    status_route: "admin_alert_kind_status",
    icon: "fa-triangle-exclamation",
    not_found: ErrorCode::AlertKindNotFound,
    words: CodedWordKeys {
        title: "admin_menu.alerts",
        sub: "admin_alert_kinds.sub",
        add: "admin_alert_kinds.add",
        none: "admin_alert_kinds.none",
        hide_title: "admin_alert_kinds.hide_title",
        what: "admin_alert_kinds.what",
        on: "admin_alert_kinds.on",
        off: "admin_alert_kinds.off",
        name: "admin_alert_kinds.name",
        name_placeholder: "admin_alert_kinds.name_placeholder",
        created: "admin_alert_kinds.created",
        hidden: "admin_alert_kinds.hidden",
        shown: "admin_alert_kinds.shown",
    },
    created: Flash::AlertKindCreated,
    saved: Flash::AlertKindSaved,
    hidden: Flash::AlertKindHidden,
    shown: Flash::AlertKindShown,
    history: HistoryOf::AlertKind,
    hide_action: "hide_alert_kind",
};

/// GET /admin/alert-kinds: the list is a tab of the alerts page.
pub async fn list() -> Result<Response, Error> {
    moved_permanently(&coded_list_href()?)
}

fn coded_list_href() -> Result<String, Error> {
    ListPosition {
        tab: TAB_KINDS.to_owned(),
        ..ListPosition::default()
    }
    .href("admin_alerts", &[])
}

/// POST /admin/alert-kinds
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<CodedForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    coded::create(&PAGES, &state, &actor, form).await
}

/// GET /admin/alert-kinds/{id}: the kind's edit sheet on the tab.
pub async fn show(ApiPath(raw_id): ApiPath<String>) -> Result<Response, Error> {
    coded::show(&PAGES, &raw_id)
}

/// POST /admin/alert-kinds/{id}: its edit sheet, back on the tab.
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiQuery(from): ApiQuery<ListPosition>,
    ApiForm(form): ApiForm<CodedForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    coded::update(&PAGES, &state, &actor, &raw_id, from, form).await
}

/// POST /admin/alert-kinds/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    coded::change_status(&PAGES, &state, &actor, &raw_id, form).await
}
