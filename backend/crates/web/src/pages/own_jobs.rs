//! /provider/jobs: jobs farmers posted near the provider's business in the
//! categories of its live services, open ones of the last `jobs.open_days`,
//! newest first; each with its distance and a call button. Linked from the
//! "a job near you" email and the dashboard.

use application::outputs::JobPostDto;
use axum::{extract::State, response::Response};
use domain::Error;
use serde::Serialize;

use crate::{
    app_state::AppState, middleware::auth::Viewer, pages::see_other, routes, shell::Shell,
    views::render_page,
};

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("own jobs links: {e}")))
}

#[derive(Serialize)]
pub struct OwnJobsPage {
    pub jobs: Vec<JobPostDto>,
    pub days: i64,
    pub dashboard_href: String,
}

/// GET /provider/jobs
pub async fn list(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login")?);
    };
    let (jobs, settings) = tokio::try_join!(
        state.usecases.job.list_own(&actor),
        state.usecases.settings.current()
    )?;
    let page = OwnJobsPage {
        jobs,
        days: settings.job_post_days(),
        dashboard_href: url("provider_dashboard")?,
    };
    render_page(&state, &shell, "pages/own_jobs.html", &page)
}
