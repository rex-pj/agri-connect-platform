//! /provider/jobs: jobs farmers posted near the provider's business in the
//! categories of its live services, open ones of the last `jobs.open_days`,
//! newest first, a page at a time; each with its distance and a call button.
//! Linked from the "a job near you" email and the dashboard.

use application::outputs::JobPostDto;
use axum::{extract::State, response::Response};
use domain::Error;
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    extract::ApiQuery,
    middleware::auth::Viewer,
    pages::{
        listing::{parse_opt, past_the_end, Pagination},
        see_other,
    },
    routes,
    shell::Shell,
    views::render_page,
};

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("own jobs links: {e}")))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
}

#[derive(Serialize)]
pub struct OwnJobsPage {
    pub jobs: Vec<JobPostDto>,
    pub total: i64,
    pub pagination: Option<Pagination>,
    pub days: i64,
    pub dashboard_href: String,
}

/// GET /provider/jobs
pub async fn list(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login")?);
    };
    let (jobs, settings) = tokio::try_join!(
        state
            .usecases
            .job
            .list_own(&actor, parse_opt(&q.page).unwrap_or(1)),
        state.usecases.settings.current()
    )?;
    let path = url("provider_jobs")?;
    if let Some(redirect) = past_the_end(&path, &[], jobs.page, jobs.total_pages) {
        return redirect;
    }
    let page = OwnJobsPage {
        pagination: Pagination::new(&path, &[], jobs.page, jobs.total_pages),
        total: jobs.total,
        jobs: jobs.items,
        days: settings.job_post_days(),
        dashboard_href: url("provider_dashboard")?,
    };
    render_page(&state, &shell, "pages/own_jobs.html", &page)
}
