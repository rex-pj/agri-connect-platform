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
        admin::{render_admin, url, AdminTab, ListPosition, SearchBox, QUEUE_SORTS},
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
    /// The words searched, the order asked.
    pub q: String,
    /// The service type the list is narrowed to.
    pub category_id: String,
    /// The area the list is narrowed to.
    pub location_id: String,
    /// How long it has waited: `stale_3`, `stale_7`.
    pub gap: String,
    pub sort: String,
    pub from: String,
    pub to: String,
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
    pub search: SearchBox,
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
    let position = ListPosition {
        page: q.page,
        state: q.state,
        q: q.q,
        sort: q.sort,
        from: q.from,
        to: q.to,
        category_id: q.category_id,
        location_id: q.location_id,
        gap: q.gap,
        ..ListPosition::default()
    }
    .checked(JOB_STATUSES, QUEUE_SORTS);
    let state_filter = position.state_in(JOB_STATUSES);
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = state
        .usecases
        .job
        .list_admin(&actor, position.page(), &position.filter(JOB_STATUSES))
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
    let (types, areas) = tokio::try_join!(
        state.usecases.job.service_type_choices(&actor),
        state.usecases.job.area_choices(&actor),
    )?;
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
        Some((JOB_STATUS_OPEN, open)),
    )?;
    let path = url("admin_jobs", &[])?;
    let body = JobsPage {
        total,
        rows,
        pagination: position.pagination(&path, page, total_pages),
        search: position
            .search_box("admin_jobs")?
            .picked(
                "category_id",
                locale.text("admin_pick.service_type"),
                locale.text("admin_pick.all_types"),
                types.into_iter().map(|t| (t.id, t.name)).collect(),
            )
            .picked(
                "location_id",
                locale.text("admin_pick.area"),
                locale.text("admin_pick.all_areas"),
                areas.into_iter().map(|a| (a.id, a.name)).collect(),
            )
            .waiting(state.locale())
            .dated(locale)
            .sorted(locale, &position, "admin_sort.newest", QUEUE_SORTS),
        states,
        narrowed: state_filter.is_some()
            || position.category_id().is_some()
            || position.location_id().is_some(),
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
    pub q: String,
    pub sort: String,
    pub from: String,
    pub to: String,
    pub category_id: String,
    pub location_id: String,
    pub gap: String,
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
        q: form.q,
        sort: form.sort,
        from: form.from,
        to: form.to,
        category_id: form.category_id,
        location_id: form.location_id,
        gap: form.gap,
        ..ListPosition::default()
    }
    .back_to("admin_jobs", Flash::JobStatusSaved)
}
