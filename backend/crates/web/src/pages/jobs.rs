//! GET /jobs/new and POST /jobs: a farmer posts a job ("phun thuốc 3 công
//! lúa"): the service it needs, where (the phone's position through the
//! `ac-geolocate` island filling two hidden boxes, else an area), what, a
//! name and a phone. Providers nearby that offer it are emailed and see it
//! on their jobs page. Post/Redirect/Get: success lands back here with a
//! flash; a broken rule re-renders the form with its messages.

use application::{
    constants::REQUESTER_NAME_MAX_LEN,
    inputs::JobInput,
    outputs::{CategoryDto, LocationDto, WardMatchDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{constants::CATEGORY_TYPE_SERVICE, Error};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiQuery},
    pages::{
        forms::{is_form_error, optional_coordinate, FormErrors},
        listing::parse_opt,
        see_other,
    },
    routes,
    shell::Shell,
    views::render_page_status,
};

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("job links: {e}")))
}

/// The form as typed.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct JobForm {
    pub category_id: String,
    /// A ward found by typing its name (`w28439`); with JS only.
    pub ward: String,
    pub location_id: String,
    pub lat: String,
    pub lng: String,
    pub description: String,
    pub requester_name: String,
    pub requester_phone: String,
}

impl JobForm {
    fn input(&self, client: String) -> JobInput {
        JobInput {
            client,
            category_id: parse_opt(&self.category_id),
            location_id: parse_opt(&self.location_id),
            ward: Some(self.ward.clone()),
            lat: optional_coordinate(&self.lat),
            lng: optional_coordinate(&self.lng),
            description: self.description.clone(),
            requester_name: self.requester_name.clone(),
            requester_phone: self.requester_phone.clone(),
        }
    }
}

/// `?category_id=` preselects the service (from a category page).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct NewQuery {
    pub category_id: String,
}

#[derive(Serialize)]
pub struct JobPage {
    pub categories: Vec<CategoryDto>,
    pub areas: Vec<LocationDto>,
    /// The ward the form came back with, named, so its box keeps it.
    pub ward_choice: Option<WardMatchDto>,
    pub form: JobForm,
    pub errors: FormErrors,
    pub action: String,
    pub back_href: String,
    pub flash: Option<Flash>,
    /// The name box's `maxlength` (the contact form's cap).
    pub name_max: usize,
}

async fn render(
    state: &AppState,
    shell: &Shell,
    form: JobForm,
    errors: FormErrors,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response, Error> {
    let explore = &state.usecases.explore;
    let (categories, areas, ward_choice) = tokio::try_join!(
        explore.get_categories(Some(CATEGORY_TYPE_SERVICE)),
        explore.get_locations(),
        state.usecases.place.ward_choice(Some(&form.ward)),
    )?;
    let page = JobPage {
        categories,
        areas,
        ward_choice,
        form,
        errors,
        action: url("job_post")?,
        back_href: url("home")?,
        flash,
        name_max: REQUESTER_NAME_MAX_LEN,
    };
    render_page_status(state, shell, status, "pages/job_new.html", &page)
}

/// GET /jobs/new
pub async fn new(
    shell: Shell,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<NewQuery>,
) -> Result<Response, Error> {
    let form = JobForm {
        category_id: q.category_id,
        ..JobForm::default()
    };
    let mut response = render(
        &state,
        &shell,
        form,
        FormErrors::default(),
        flash.0,
        StatusCode::OK,
    )
    .await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /jobs
pub async fn create(
    shell: Shell,
    ip: Option<axum::Extension<crate::middleware::client_ip::ClientIp>>,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<JobForm>,
) -> Result<Response, Error> {
    let client = crate::middleware::client_ip::client_key(ip.as_deref());
    match state.usecases.job.post(form.input(client)).await {
        Ok(_) => {
            let mut response = see_other(&url("job_new")?)?;
            Flash::JobPosted.set_on(&mut response);
            Ok(response)
        }
        Err(err) if is_form_error(&err) => {
            let errors = FormErrors::from_error(&state, &err);
            render(&state, &shell, form, errors, None, err.kind().status()).await
        }
        Err(err) => Err(err),
    }
}
