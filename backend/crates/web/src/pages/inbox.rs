//! /provider/inquiries — the contact requests farmers sent to the provider's
//! business, newest first, each with a call button and its handling status —
//! and POST /provider/inquiries/{id}/status.
//!
//! Each card ends with the next steps of its status (`status_steps`, the
//! same buttons as the admin's list): one click each, but spam, whose link
//! reloads the inbox with `?spam={id}` and that card's confirmation open.

use std::collections::BTreeMap;

use application::outputs::OwnInquiryDto;
use axum::{extract::State, response::Response};
use domain::{constants::INQUIRY_STATUSES_REVIEWABLE, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        canonical_url,
        listing::{parse_opt, past_the_end, with_query, Pagination},
        parse_id,
        provider::REVIEWS_ANCHOR,
        see_other,
        status_steps::{inquiry_steps, offers_spam, Step},
    },
    routes,
    shell::Shell,
    views::render_page,
};

fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("inbox links: {e}")))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    /// The inquiry whose spam confirmation is open.
    pub spam: String,
}

#[derive(Serialize)]
pub struct InquiryRow {
    pub inquiry: OwnInquiryDto,
    pub action: String,
    /// The buttons this card offers, from its status.
    pub steps: &'static [Step],
    pub spam_href: String,
    /// The link to review this business, offered once the farmer was
    /// called back (`contacted`, `closed`): the provider sends it on Zalo.
    pub review_href: Option<String>,
}

/// The spam confirmation: the farmer's name and where the form posts.
#[derive(Serialize)]
pub struct Confirm {
    pub requester: String,
    pub action: String,
}

#[derive(Serialize)]
pub struct InboxPage {
    pub rows: Vec<InquiryRow>,
    pub total: i64,
    pub new_count: i64,
    pub pagination: Option<Pagination>,
    /// Sent back with a card's form (`page`), so saving returns to the
    /// same page.
    pub keep: BTreeMap<&'static str, String>,
    /// The spam confirmation open on this render.
    pub confirm: Option<Confirm>,
    pub dashboard_href: String,
    /// For the review link's message.
    pub business_name: String,
    pub flash: Option<Flash>,
}

/// GET /provider/inquiries
pub async fn list(
    shell: Shell,
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let inbox = state
        .usecases
        .provider_dashboard
        .list_own_inquiries(&actor, parse_opt(&q.page).unwrap_or(1))
        .await?;
    let path = url("provider_inquiries", &[])?;
    if let Some(redirect) = past_the_end(&path, &[], inbox.page, inbox.total_pages) {
        return redirect;
    }
    // The review link: the provider's page at its reviews.
    let profile = state
        .usecases
        .provider_dashboard
        .get_profile(&actor)
        .await?;
    let review_href = format!(
        "{}#{REVIEWS_ANCHOR}",
        canonical_url("provider", profile.id, &profile.name)?
    );
    let business_name = profile.name;
    let page_param = Some(inbox.page)
        .filter(|p| *p > 1)
        .map(|p| p.to_string())
        .unwrap_or_default();
    let spam = parse_opt::<i32>(&q.spam);
    let mut confirm = None;
    let mut rows = Vec::with_capacity(inbox.items.len());
    for inquiry in inbox.items {
        let id = inquiry.id.to_string();
        let action = url("provider_inquiry_status", &[("id", &id)])?;
        let steps = inquiry_steps(&inquiry.status);
        if spam == Some(inquiry.id) && offers_spam(steps) {
            confirm = Some(Confirm {
                requester: inquiry.requester_name.clone(),
                action: action.clone(),
            });
        }
        rows.push(InquiryRow {
            spam_href: with_query(&path, &[("page", &page_param), ("spam", &id)]),
            action,
            steps,
            review_href: INQUIRY_STATUSES_REVIEWABLE
                .contains(&inquiry.status.as_str())
                .then(|| review_href.clone()),
            inquiry,
        });
    }
    let page = InboxPage {
        rows,
        total: inbox.total,
        new_count: inbox.new_count,
        pagination: Pagination::new(&path, &[], inbox.page, inbox.total_pages),
        keep: BTreeMap::from([("page", inbox.page.to_string())]),
        confirm,
        dashboard_href: url("provider_dashboard", &[])?,
        business_name,
        flash: flash.0,
    };
    let mut response = render_page(&state, &shell, "pages/inbox.html", &page)?;
    flash.clear_on(&mut response);
    Ok(response)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    pub status: String,
    pub page: String,
}

/// POST /provider/inquiries/{id}/status
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
        .provider_dashboard
        .set_own_inquiry_status(&actor, id, form.status.trim())
        .await?;
    // Rebuilt from a number, never a posted URL.
    let page = parse_opt::<i64>(&form.page)
        .filter(|p| *p > 1)
        .map(|p| p.to_string())
        .unwrap_or_default();
    let mut response = see_other(&with_query(
        &url("provider_inquiries", &[])?,
        &[("page", &page)],
    ))?;
    Flash::InquiryStatusSaved.set_on(&mut response);
    Ok(response)
}
