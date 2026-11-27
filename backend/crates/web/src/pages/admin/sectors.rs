//! /admin/sectors and /admin/sectors/{id}: the sectors the platform serves
//! (ngành: trồng trọt, chăn nuôi, thuỷ sản), the top of the catalogue and the
//! first tab of its screen (`catalogue_tabs`). The sector switch, the
//! commodities, the service and supply types and the alerts name them by code.
//! The pages are the shared
//! coded-list pages (`coded`); this module says where and in which words.

use application::usecases::admin_usecase::HistoryOf;
use axum::{extract::State, response::Response};
use domain::{Error, ErrorCode};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{
            catalogue_tabs,
            coded::{self, CodedForm, CodedPages, CodedWordKeys, ListQuery, ListView, StatusForm},
            url, CatalogueTab, ListPosition,
        },
        see_other,
    },
};

pub const PAGES: CodedPages = CodedPages {
    usecase: |state| &state.usecases.sector,
    list_route: "admin_sectors",
    list_tab: "",
    tabs: Some(|state, _| catalogue_tabs(state, CatalogueTab::Sectors)),
    create_route: "admin_sectors",
    entry_route: "admin_sector",
    status_route: "admin_sector_status",
    icon: "fa-layer-group",
    not_found: ErrorCode::SectorNotFound,
    words: CodedWordKeys {
        title: "admin_menu.sectors",
        sub: "admin_sectors.sub",
        add: "admin_sectors.add",
        none: "admin_sectors.none",
        hide_title: "admin_sectors.hide_title",
        what: "admin_sectors.what",
        on: "admin_sectors.on",
        off: "admin_sectors.off",
        name: "admin_sectors.name",
        name_placeholder: "admin_sectors.name_placeholder",
        created: "admin_sectors.created",
        hidden: "admin_sectors.hidden",
        shown: "admin_sectors.shown",
    },
    created: Flash::SectorCreated,
    saved: Flash::SectorSaved,
    hidden: Flash::SectorHidden,
    shown: Flash::SectorShown,
    history: HistoryOf::Sector,
    hide_action: "hide_sector",
};

/// GET /admin/sectors
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let view = ListView::asked(&q, flash.0);
    let mut response = coded::list_page(&PAGES, &state, &actor, view).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/sectors
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

/// GET /admin/sectors/{id}: the sector's edit sheet on the list.
pub async fn show(ApiPath(raw_id): ApiPath<String>) -> Result<Response, Error> {
    coded::show(&PAGES, &raw_id)
}

/// POST /admin/sectors/{id}: its edit sheet, back on the list position
/// of its query.
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

/// POST /admin/sectors/{id}/status
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
