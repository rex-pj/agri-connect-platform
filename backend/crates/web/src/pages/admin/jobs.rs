//! /admin/jobs: every job post farmers made, newest first, each with its
//! state (open, closed, spam) as a badge. The row ends with the next steps
//! only (`status_steps`: done, spam, reopen), each one click but spam, whose
//! link reloads the list with `?spam={id}` and that row's confirmation open
//! (with a reason for the audit log). Chips narrow the list to one state
//! (`?state=`); every form lands back on the same page of the list.

use application::outputs::{AdminPagedResponse, JobPostDto};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{
    constants::{JOB_STATUSES, JOB_STATUS_OPEN},
    Error, ErrorCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{render_admin, url, AdminTab, ListPosition},
        forms::reason,
        listing::{parse_opt, Pagination},
        parse_id, see_other,
        status_steps::{job_status_key, job_steps, offers_spam, Step},
    },
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    /// The state the list is narrowed to.
    pub state: String,
    /// The post whose spam confirmation is open.
    pub spam: String,
}

#[derive(Serialize)]
pub struct JobRow {
    pub job: JobPostDto,
    pub action: String,
    /// The buttons this row offers, from its state.
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
pub struct JobsPage {
    pub total: i64,
    pub rows: Vec<JobRow>,
    pub pagination: Option<Pagination>,
    /// The state chips above the list.
    pub states: Vec<AdminTab>,
    /// The list is narrowed to one state.
    pub narrowed: bool,
    pub confirm: Option<Confirm>,
    pub position: ListPosition,
    pub flash: Option<Flash>,
}

/// GET /admin/jobs
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
        state: q.state,
        ..ListPosition::default()
    };
    let state_filter = position.state_in(JOB_STATUSES);
    position.state = state_filter.unwrap_or_default().to_owned();
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = state
        .usecases
        .job
        .list_admin(&actor, position.page(), state_filter)
        .await?;
    let position = ListPosition {
        page: page.to_string(),
        ..position
    };
    let spam = parse_opt::<i32>(&q.spam);
    let mut confirm = None;
    let mut rows = Vec::with_capacity(items.len());
    for job in items {
        let id = job.id.to_string();
        let action = url("admin_job_status", &[("id", &id)])?;
        let steps = job_steps(&job.status);
        if spam == Some(job.id) && offers_spam(steps) {
            confirm = Some(Confirm {
                requester: job.requester_name.clone(),
                action: action.clone(),
            });
        }
        rows.push(JobRow {
            spam_href: position.href("admin_jobs", &[("spam", &id)])?,
            action,
            steps,
            job,
        });
    }
    let open = state
        .usecases
        .job
        .count_admin(&actor, JOB_STATUS_OPEN)
        .await?;
    let locale = state.locale();
    let states = position.state_chips(
        "admin_jobs",
        JOB_STATUSES,
        locale.text("filters.all"),
        |s| locale.text(job_status_key(s)),
        (JOB_STATUS_OPEN, open),
    )?;
    let path = url("admin_jobs", &[])?;
    let body = JobsPage {
        total,
        rows,
        pagination: position.pagination(&path, page, total_pages),
        states,
        narrowed: state_filter.is_some(),
        confirm,
        position,
        flash: flash.0,
    };
    let mut response = render_admin(
        &state,
        &actor,
        &path,
        StatusCode::OK,
        "pages/admin/jobs.html",
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
    /// The list page and state to land back on.
    pub page: String,
    pub state: String,
}

/// POST /admin/jobs/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::JobNotFound)?;
    state
        .usecases
        .job
        .set_status(&actor, id, &form.status, reason(&form.reason))
        .await?;
    ListPosition {
        page: form.page,
        state: form.state,
        ..ListPosition::default()
    }
    .back_to("admin_jobs", Flash::JobStatusSaved)
}
