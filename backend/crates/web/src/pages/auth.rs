//! /login, /register, /forgot-password, /reset-password: plain forms that
//! post back to themselves (Post/Redirect/Get), on the standalone auth layout.
//!
//! - Signing in starts a browser session (`web::session`) and lands where the
//!   visitor's permissions point (bug 0.1: it used to be the admin dashboard
//!   for everyone).
//! - /login sends a fresh installation to /setup (bug 0.2).
//! - Passwords are never echoed back into a re-shown form.

use application::authz::Actor;
use axum::{extract::State, response::Response, Extension};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        forms::{confirm_password, is_form_error, FormErrors},
        see_other,
    },
    routes,
    session::CookiePolicy,
    shell::{home_area_href, Shell},
    views::{render_page, render_page_status},
};

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("auth links: {e}")))
}

/// Auth pages have no app frame; the shell only carries the path.
fn bare_shell(state: &AppState, path: &str) -> Shell {
    Shell::without_session(state.locale(), path, None)
}

/// Where a signed-in visitor belongs: the same "my area" as the last tab.
pub fn landing(actor: Option<&Actor>) -> String {
    home_area_href(actor)
}

/// 303 to the visitor's area with a fresh session in the cookies.
pub async fn signed_in(
    state: &AppState,
    policy: CookiePolicy,
    user_id: i32,
    flash: Option<Flash>,
) -> Result<Response, Error> {
    let tokens = state.usecases.auth.start_session(user_id).await?;
    let actor = state.usecases.auth.actor(user_id).await?;
    let mut response = see_other(&landing(actor.as_ref()))?;
    policy.set(&mut response, &tokens);
    if let Some(flash) = flash {
        flash.set_on(&mut response);
    }
    Ok(response)
}

/// Re-shows `template` with the error's status, or passes server failures on
/// to the error page.
fn reshow<T: Serialize>(
    state: &AppState,
    path: &str,
    err: Error,
    template: &str,
    page: impl FnOnce(FormErrors) -> T,
) -> Result<Response, Error> {
    if !is_form_error(&err) {
        return Err(err);
    }
    let errors = FormErrors::from_error(state, &err);
    render_page_status(
        state,
        &bare_shell(state, path),
        err.kind().status(),
        template,
        &page(errors),
    )
}

// ── Login ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginPage {
    pub username: String,
    pub errors: FormErrors,
    /// Too many wrong passwords: the lockout panel replaces the form.
    pub locked: bool,
    /// Just reset the password (flash from /reset-password).
    pub reset: bool,
}

pub async fn login_page(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    if viewer.is_some() {
        return see_other(&landing(viewer.as_ref()));
    }
    if state.usecases.auth.needs_first_admin().await? {
        return see_other(&url("setup")?);
    }
    let page = LoginPage {
        username: String::new(),
        errors: FormErrors::default(),
        locked: false,
        reset: flash.0 == Some(Flash::PasswordReset),
    };
    let mut response = render_page(
        &state,
        &bare_shell(&state, &url("login")?),
        "pages/login.html",
        &page,
    )?;
    flash.clear_on(&mut response);
    Ok(response)
}

pub async fn login(
    State(state): State<AppState>,
    Extension(policy): Extension<CookiePolicy>,
    ApiForm(form): ApiForm<LoginForm>,
) -> Result<Response, Error> {
    match state
        .usecases
        .auth
        .login(form.username.clone(), form.password)
        .await
    {
        Ok(login) => signed_in(&state, policy, login.user_id, None).await,
        Err(err) => {
            let locked = err.code() == ErrorCode::AccountLocked;
            reshow(&state, &url("login")?, err, "pages/login.html", |errors| {
                LoginPage {
                    username: form.username,
                    errors,
                    locked,
                    reset: false,
                }
            })
        }
    }
}

// ── Register ──────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RegisterForm {
    pub business_name: String,
    pub business_phone: String,
    pub full_name: String,
    pub email: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    #[serde(skip_serializing)]
    pub password2: String,
}

#[derive(Serialize)]
pub struct RegisterPage {
    pub values: RegisterForm,
    pub errors: FormErrors,
}

pub async fn register_page(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    if viewer.is_some() {
        return see_other(&landing(viewer.as_ref()));
    }
    let page = RegisterPage {
        values: RegisterForm::default(),
        errors: FormErrors::default(),
    };
    render_page(
        &state,
        &bare_shell(&state, &url("register")?),
        "pages/register.html",
        &page,
    )
}

pub async fn register(
    State(state): State<AppState>,
    Extension(policy): Extension<CookiePolicy>,
    ApiForm(form): ApiForm<RegisterForm>,
) -> Result<Response, Error> {
    let result = match confirm_password(&form.password, &form.password2, "password2") {
        Err(e) => Err(e),
        Ok(()) => {
            let full_name = Some(form.full_name.trim().to_owned()).filter(|n| !n.is_empty());
            state
                .usecases
                .auth
                .register(
                    form.username.clone(),
                    form.email.clone(),
                    form.password.clone(),
                    full_name,
                    form.business_name.clone(),
                    form.business_phone.clone(),
                )
                .await
        }
    };
    match result {
        Ok(login) => signed_in(&state, policy, login.user_id, None).await,
        Err(err) => reshow(
            &state,
            &url("register")?,
            err,
            "pages/register.html",
            |errors| RegisterPage {
                values: form,
                errors,
            },
        ),
    }
}

// ── Forgot password ───────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ForgotForm {
    pub email: String,
}

#[derive(Serialize)]
pub struct ForgotPage {
    /// The request went through: "check your inbox", whether or not the
    /// address exists (anything else would reveal who has an account).
    pub sent: bool,
}

pub async fn forgot_page(State(state): State<AppState>) -> Result<Response, Error> {
    render_page(
        &state,
        &bare_shell(&state, &url("forgot_password")?),
        "pages/forgot_password.html",
        &ForgotPage { sent: false },
    )
}

pub async fn forgot(
    State(state): State<AppState>,
    ApiForm(form): ApiForm<ForgotForm>,
) -> Result<Response, Error> {
    state.usecases.auth.forgot_password(form.email).await?;
    render_page(
        &state,
        &bare_shell(&state, &url("forgot_password")?),
        "pages/forgot_password.html",
        &ForgotPage { sent: true },
    )
}

// ── Reset password ────────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ResetQuery {
    pub token: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ResetForm {
    pub token: String,
    pub new_password: String,
    pub password2: String,
}

#[derive(Serialize)]
pub struct ResetPage {
    pub token: String,
    /// No token, or one that is expired or already used.
    pub invalid: bool,
    pub errors: FormErrors,
}

pub async fn reset_page(
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ResetQuery>,
) -> Result<Response, Error> {
    let token = q.token.trim().to_owned();
    let page = ResetPage {
        invalid: token.is_empty(),
        token,
        errors: FormErrors::default(),
    };
    render_page(
        &state,
        &bare_shell(&state, &url("reset_password")?),
        "pages/reset_password.html",
        &page,
    )
}

pub async fn reset(
    State(state): State<AppState>,
    ApiForm(form): ApiForm<ResetForm>,
) -> Result<Response, Error> {
    let result = match confirm_password(&form.new_password, &form.password2, "password2") {
        Err(e) => Err(e),
        Ok(()) => {
            state
                .usecases
                .auth
                .reset_password(form.token.clone(), form.new_password)
                .await
        }
    };
    match result {
        Ok(()) => {
            let mut response = see_other(&url("login")?)?;
            Flash::PasswordReset.set_on(&mut response);
            Ok(response)
        }
        Err(err) => {
            let invalid = err.code() == ErrorCode::ResetTokenInvalid;
            reshow(
                &state,
                &url("reset_password")?,
                err,
                "pages/reset_password.html",
                |errors| ResetPage {
                    token: form.token,
                    invalid,
                    errors,
                },
            )
        }
    }
}
