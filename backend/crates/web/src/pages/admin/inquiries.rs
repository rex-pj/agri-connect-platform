//! /admin/inquiries: contact requests farmers sent to providers, newest
//! first, each with its handling status (new → contacted → closed, or spam).
//!
//! Each row is a small form: a status select and a save button. With JS the
//! select submits itself (Alpine `autoSubmit`); without, the button does.
//! The list can be narrowed to one provider (`?provider_id=`, linked from the
//! provider's page); every form keeps the list position.

use application::{
    catalogue::PERM_INQUIRY_MANAGE,
    outputs::{AdminInquiryDto, AdminPagedResponse},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{constants::INQUIRY_STATUSES, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{render_admin, url, ListPosition, ProviderFilter},
        listing::Pagination,
        parse_id, see_other,
    },
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    pub provider_id: String,
}

#[derive(Serialize)]
pub struct InquiryRow {
    pub inquiry: AdminInquiryDto,
    pub provider_href: String,
    pub action: String,
}

#[derive(Serialize)]
pub struct InquiriesPage {
    pub total: i64,
    pub rows: Vec<InquiryRow>,
    pub pagination: Option<Pagination>,
    /// The codes the select offers; the template names them.
    pub statuses: &'static [&'static str],
    pub can_manage: bool,
    /// Set when the list is one provider's.
    pub filter: Option<ProviderFilter>,
    pub position: ListPosition,
    pub flash: Option<Flash>,
}

/// GET /admin/inquiries
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let position = ListPosition {
        page: q.page,
        provider_id: q.provider_id,
        ..ListPosition::default()
    };
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = state
        .usecases
        .admin
        .list_inquiries(&actor, position.page(), position.provider_id())
        .await?;
    let position = ListPosition {
        page: page.to_string(),
        ..position
    };
    let mut rows = Vec::with_capacity(items.len());
    for inquiry in items {
        rows.push(InquiryRow {
            provider_href: url(
                "admin_provider",
                &[("id", &inquiry.business_id.to_string())],
            )?,
            action: url("admin_inquiry_status", &[("id", &inquiry.id.to_string())])?,
            inquiry,
        });
    }
    let path = url("admin_inquiries", &[])?;
    let name = rows.first().map(|r| r.inquiry.business_name.clone());
    let body = InquiriesPage {
        total,
        rows,
        pagination: position.pagination(&path, page, total_pages),
        statuses: INQUIRY_STATUSES,
        can_manage: actor.can(PERM_INQUIRY_MANAGE),
        filter: position.provider_filter("admin_inquiries", name)?,
        position,
        flash: flash.0,
    };
    let mut response = render_admin(
        &state,
        &actor,
        &path,
        StatusCode::OK,
        "pages/admin/inquiries.html",
        &body,
    )?;
    flash.clear_on(&mut response);
    Ok(response)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    pub status: String,
    pub page: String,
    pub provider_id: String,
}

/// POST /admin/inquiries/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::InquiryNotFound)?;
    state
        .usecases
        .admin
        .update_inquiry_status(&actor, id, form.status)
        .await?;
    ListPosition {
        page: form.page,
        provider_id: form.provider_id,
        ..ListPosition::default()
    }
    .back_to("admin_inquiries", Flash::InquiryStatusSaved)
}
