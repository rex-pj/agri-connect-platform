//! /admin/inquiries: contact requests farmers sent to providers, newest
//! first, each with its handling status (new → contacted → closed, or spam).
//!
//! The status is a badge; the row ends with the next steps only
//! (`status_steps`: called back, done, spam, reopen), each one click but
//! spam, whose link reloads the list with `?spam={id}` and that row's
//! confirmation open (with a reason for the audit log). Chips narrow the list
//! to one status (`?state=`), and it can be narrowed to one provider
//! (`?provider_id=`, linked from the provider's page); every form keeps the
//! list position.

use application::{
    catalogue::PERM_INQUIRY_MANAGE,
    outputs::{AdminInquiryDto, AdminPagedResponse},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{
    constants::{INQUIRY_STATUSES, INQUIRY_STATUS_NEW},
    Error, ErrorCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{render_admin, url, AdminTab, ListPosition, ProviderFilter},
        forms::reason,
        listing::{parse_opt, Pagination},
        parse_id, see_other,
        status_steps::{inquiry_status_key, inquiry_steps, offers_spam, Step},
    },
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    pub provider_id: String,
    /// The status the list is narrowed to.
    pub state: String,
    /// The inquiry whose spam confirmation is open.
    pub spam: String,
}

#[derive(Serialize)]
pub struct InquiryRow {
    pub inquiry: AdminInquiryDto,
    pub provider_href: String,
    pub action: String,
    /// The buttons this row offers, from its status.
    pub steps: &'static [Step],
    pub spam_href: String,
}

/// The spam confirmation open on this render.
#[derive(Serialize)]
pub struct Confirm {
    pub requester: String,
    pub action: String,
}

#[derive(Serialize)]
pub struct InquiriesPage {
    pub total: i64,
    pub rows: Vec<InquiryRow>,
    pub pagination: Option<Pagination>,
    /// The status chips above the list.
    pub states: Vec<AdminTab>,
    /// The list is narrowed to one status.
    pub narrowed: bool,
    pub can_manage: bool,
    pub confirm: Option<Confirm>,
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
    let mut position = ListPosition {
        page: q.page,
        provider_id: q.provider_id,
        state: q.state,
        ..ListPosition::default()
    };
    let state_filter = position.state_in(INQUIRY_STATUSES);
    position.state = state_filter.unwrap_or_default().to_owned();
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = state
        .usecases
        .admin
        .list_inquiries(
            &actor,
            position.page(),
            position.provider_id(),
            state_filter,
        )
        .await?;
    let position = ListPosition {
        page: page.to_string(),
        ..position
    };
    let can_manage = actor.can(PERM_INQUIRY_MANAGE);
    let spam = parse_opt::<i32>(&q.spam).filter(|_| can_manage);
    let mut confirm = None;
    let mut rows = Vec::with_capacity(items.len());
    for inquiry in items {
        let id = inquiry.id.to_string();
        let action = url("admin_inquiry_status", &[("id", &id)])?;
        let steps = inquiry_steps(&inquiry.status);
        if spam == Some(inquiry.id) && offers_spam(steps) {
            confirm = Some(Confirm {
                requester: inquiry.requester_name.clone(),
                action: action.clone(),
            });
        }
        rows.push(InquiryRow {
            provider_href: url(
                "admin_provider",
                &[("id", &inquiry.business_id.to_string())],
            )?,
            spam_href: position.href("admin_inquiries", &[("spam", &id)])?,
            action,
            steps,
            inquiry,
        });
    }
    let new = state
        .usecases
        .admin
        .count_inquiries(&actor, position.provider_id(), INQUIRY_STATUS_NEW)
        .await?;
    let locale = state.locale();
    let states = position.state_chips(
        "admin_inquiries",
        INQUIRY_STATUSES,
        locale.text("filters.all"),
        |s| locale.text(inquiry_status_key(s)),
        (INQUIRY_STATUS_NEW, new),
    )?;
    let path = url("admin_inquiries", &[])?;
    let name = rows.first().map(|r| r.inquiry.business_name.clone());
    let body = InquiriesPage {
        total,
        rows,
        pagination: position.pagination(&path, page, total_pages),
        states,
        narrowed: state_filter.is_some(),
        can_manage,
        confirm,
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
    /// Why (asked when marking spam), for the audit log.
    pub reason: String,
    pub page: String,
    pub provider_id: String,
    pub state: String,
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
        .update_inquiry_status(&actor, id, form.status, reason(&form.reason))
        .await?;
    ListPosition {
        page: form.page,
        provider_id: form.provider_id,
        state: form.state,
        ..ListPosition::default()
    }
    .back_to("admin_inquiries", Flash::InquiryStatusSaved)
}
