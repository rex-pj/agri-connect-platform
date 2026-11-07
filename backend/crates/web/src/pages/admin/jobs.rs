//! /admin/jobs: every job post farmers made, newest first, each with its
//! state (open, closed, spam). Each row's state is a small form: with JS the
//! select saves itself (`autoSubmit`), without JS its button does; it lands
//! back on the same page of the list.

use application::outputs::{AdminPagedResponse, JobPostDto};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{constants::JOB_STATUSES, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{render_admin, url, ListPosition},
        listing::Pagination,
        parse_id, see_other,
    },
};

/// A state for the select: its code and its word.
#[derive(Serialize)]
pub struct StatusOption {
    pub value: &'static str,
    pub label: String,
}

/// The word of a job state (`job_status.<code>`), from the one list.
fn status_options(state: &AppState) -> Vec<StatusOption> {
    JOB_STATUSES
        .iter()
        .map(|s| StatusOption {
            value: s,
            label: state.locale().text(match *s {
                "open" => "job_status.open",
                "closed" => "job_status.closed",
                _ => "job_status.spam",
            }),
        })
        .collect()
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
}

#[derive(Serialize)]
pub struct JobRow {
    pub job: JobPostDto,
    pub action: String,
}

#[derive(Serialize)]
pub struct JobsPage {
    pub total: i64,
    pub rows: Vec<JobRow>,
    pub pagination: Option<Pagination>,
    pub statuses: Vec<StatusOption>,
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
    let position = ListPosition {
        page: q.page,
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
        .job
        .list_admin(&actor, position.page())
        .await?;
    let position = ListPosition {
        page: page.to_string(),
        ..position
    };
    let rows = items
        .into_iter()
        .map(|job| {
            Ok(JobRow {
                action: url("admin_job_status", &[("id", &job.id.to_string())])?,
                job,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let path = url("admin_jobs", &[])?;
    let body = JobsPage {
        total,
        rows,
        pagination: position.pagination(&path, page, total_pages),
        statuses: status_options(&state),
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
    /// The list page to land back on.
    pub page: String,
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
        .set_status(&actor, id, &form.status, None)
        .await?;
    ListPosition {
        page: form.page,
        ..ListPosition::default()
    }
    .back_to("admin_jobs", Flash::JobStatusSaved)
}
