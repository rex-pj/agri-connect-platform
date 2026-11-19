//! /setup — the first admin, with an optional demo data load — and
//! /setup/seed — the demo data page (load and remove), for whoever holds
//! `system.demo.manage`.

use application::outputs::SetupStatusDto;
use axum::{extract::State, response::Response, Extension};
use domain::{Error, ErrorCode, Validation};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::ApiForm,
    middleware::auth::Viewer,
    pages::{
        admin::render_admin,
        auth::signed_in,
        forms::{confirm_password, is_form_error, FormErrors},
        see_other,
    },
    routes,
    session::CookiePolicy,
    shell::Shell,
    views::{render_page, render_page_status},
};

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("setup links: {e}")))
}

// ── First admin ───────────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SetupForm {
    pub full_name: String,
    pub email: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    #[serde(skip_serializing)]
    pub password2: String,
    /// SETUP_TOKEN as typed; never echoed back.
    #[serde(skip_serializing)]
    pub setup_token: String,
    /// The "load demo data" checkbox: present ("on") only when ticked.
    pub demo: Option<String>,
}

#[derive(Serialize)]
pub struct SetupPage {
    pub values: SetupForm,
    pub errors: FormErrors,
    /// The server has a SETUP_TOKEN: the form asks for it.
    pub token_required: bool,
}

pub async fn setup_page(State(state): State<AppState>) -> Result<Response, Error> {
    if !state.usecases.auth.needs_first_admin().await? {
        return see_other(&url("login")?);
    }
    let page = SetupPage {
        values: SetupForm::default(),
        errors: FormErrors::default(),
        token_required: state.usecases.auth.setup_token_required(),
    };
    render_page(
        &state,
        &Shell::without_session(state.locale(), &url("setup")?, None),
        "pages/setup.html",
        &page,
    )
}

pub async fn setup(
    State(state): State<AppState>,
    Extension(policy): Extension<CookiePolicy>,
    ApiForm(form): ApiForm<SetupForm>,
) -> Result<Response, Error> {
    let auth = &state.usecases.auth;
    let created = match confirm_password(&form.password, &form.password2, "password2") {
        Err(e) => Err(e),
        Ok(()) => {
            let full_name = Some(form.full_name.trim().to_owned()).filter(|n| !n.is_empty());
            auth.create_first_admin(
                form.username.clone(),
                form.email.clone(),
                form.password.clone(),
                full_name,
                Some(form.setup_token.clone()),
            )
            .await
        }
    };
    let login = match created {
        Ok(login) => login,
        // Someone finished setup first: there is nothing left to set up.
        Err(err) if err.code() == ErrorCode::AdminAlreadyExists => {
            return see_other(&url("login")?)
        }
        Err(err) if is_form_error(&err) => {
            let errors = FormErrors::from_error(&state, &err);
            let page = SetupPage {
                values: form,
                errors,
                token_required: auth.setup_token_required(),
            };
            return render_page_status(
                &state,
                &Shell::without_session(state.locale(), &url("setup")?, None),
                err.kind().status(),
                "pages/setup.html",
                &page,
            );
        }
        Err(err) => return Err(err),
    };

    let mut flash = None;
    if form.demo.is_some() {
        let actor = auth
            .actor(login.user_id)
            .await?
            .ok_or_else(|| Error::internal("the new admin has no actor"))?;
        // The admin exists either way; a failed load is retried from the
        // demo data page instead of failing the whole setup.
        match state.usecases.setup.load_demo(&actor).await {
            Ok(_) => flash = Some(Flash::DemoLoaded),
            Err(e) => tracing::warn!(error = %e, "demo data not loaded during setup"),
        }
    }
    signed_in(&state, policy, login.user_id, flash).await
}

// ── Demo data page ────────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SeedForm {
    /// "load" or "remove".
    pub action: String,
    /// The remove confirmation checkbox.
    pub confirm: Option<String>,
}

#[derive(Serialize)]
pub struct SeedPage {
    pub status: SetupStatusDto,
    pub errors: FormErrors,
    pub flash: Option<Flash>,
}

async fn seed_page(
    state: &AppState,
    actor: &application::authz::Actor,
    errors: FormErrors,
    flash: Option<Flash>,
    status: axum::http::StatusCode,
) -> Result<Response, Error> {
    let page = SeedPage {
        status: state.usecases.setup.demo_status(actor).await?,
        errors,
        flash,
    };
    render_admin(
        state,
        actor,
        &url("setup_seed")?,
        status,
        "pages/setup_seed.html",
        &page,
    )
}

pub async fn seed(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login")?);
    };
    let mut response = seed_page(
        &state,
        &actor,
        FormErrors::default(),
        flash.0,
        axum::http::StatusCode::OK,
    )
    .await?;
    flash.clear_on(&mut response);
    Ok(response)
}

pub async fn seed_action(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<SeedForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login")?);
    };
    let setup = &state.usecases.setup;
    let result = match form.action.as_str() {
        "load" => setup.load_demo(&actor).await.map(|_| Flash::DemoLoaded),
        "remove" => match Validation::new()
            .check(
                form.confirm.is_none(),
                "confirm",
                ErrorCode::ConfirmationRequired,
            )
            .finish()
        {
            Ok(()) => setup.remove_demo(&actor).await.map(|_| Flash::DemoRemoved),
            Err(e) => Err(e),
        },
        _ => Err(Error::new(ErrorCode::InvalidRequestBody)),
    };
    match result {
        Ok(flash) => {
            let mut response = see_other(&url("setup_seed")?)?;
            flash.set_on(&mut response);
            Ok(response)
        }
        // Forbidden means no permission: the error page, not the form.
        Err(err) if is_form_error(&err) && err.code() != ErrorCode::PermissionDenied => {
            let errors = FormErrors::from_error(&state, &err);
            seed_page(&state, &actor, errors, None, err.kind().status()).await
        }
        Err(err) => Err(err),
    }
}
