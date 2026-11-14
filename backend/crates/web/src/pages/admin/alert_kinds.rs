//! The kinds of alert (dịch bệnh, xâm nhập mặn, thời tiết): the alerts
//! page's second tab (`/admin/alerts?tab=kinds`, `alerts::list` hands it
//! here), their create sheet (POST /admin/alert-kinds) and one kind's page
//! (/admin/alert-kinds/{id}, lighting the alerts entry: `MENU_OWNERS`).
//! `/admin/alert-kinds` 301s to the tab. The pages are the shared
//! coded-list pages (`coded`); this module says where and in which words.

use axum::{extract::State, response::Response};
use domain::{Error, ErrorCode};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath},
    middleware::auth::Viewer,
    pages::{
        admin::{
            alerts,
            coded::{self, CodedForm, CodedPages, CodedWordKeys, StatusForm},
            url,
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
    related_route: "admin_alerts",
    related_icon: "fa-triangle-exclamation",
    icon: "fa-triangle-exclamation",
    not_found: ErrorCode::AlertKindNotFound,
    words: CodedWordKeys {
        title: "admin_menu.alerts",
        sub: "admin_alert_kinds.sub",
        add: "admin_alert_kinds.add",
        none: "admin_alert_kinds.none",
        hide: "admin_alert_kinds.hide",
        hide_title: "admin_alert_kinds.hide_title",
        what: "admin_alert_kinds.what",
        on: "admin_alert_kinds.on",
        off: "admin_alert_kinds.off",
        name: "admin_alert_kinds.name",
        name_placeholder: "admin_alert_kinds.name_placeholder",
        icon_placeholder: "admin_alert_kinds.icon_placeholder",
        related: "admin_menu.alerts",
        created: "admin_alert_kinds.created",
        hidden: "admin_alert_kinds.hidden",
        shown: "admin_alert_kinds.shown",
    },
    created: Flash::AlertKindCreated,
    saved: Flash::AlertKindSaved,
    hidden: Flash::AlertKindHidden,
    shown: Flash::AlertKindShown,
};

/// GET /admin/alert-kinds: the list is a tab of the alerts page.
pub async fn list() -> Result<Response, Error> {
    moved_permanently(&coded_list_href()?)
}

fn coded_list_href() -> Result<String, Error> {
    crate::pages::admin::ListPosition {
        tab: TAB_KINDS.to_owned(),
        ..crate::pages::admin::ListPosition::default()
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

/// GET /admin/alert-kinds/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let mut response = coded::show(&PAGES, &state, &actor, &raw_id, flash.0).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/alert-kinds/{id}
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<CodedForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    coded::update(&PAGES, &state, &actor, &raw_id, form).await
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
