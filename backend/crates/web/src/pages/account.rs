//! GET /account — who you are, and the security section — and
//! POST /account/password — change the password.
//!
//! Changing the password ends every session of the account (the use case
//! revokes them); this browser gets a fresh one so it stays signed in.

use application::outputs::MeResponse;
use axum::{extract::State, response::Response, Extension};
use domain::Error;
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::ApiForm,
    middleware::auth::Viewer,
    pages::{
        forms::{confirm_password, is_form_error, FormErrors},
        see_other,
    },
    routes,
    session::CookiePolicy,
    shell::{home_area, Link, Shell},
    views::{render_page, render_page_status},
};

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("account links: {e}")))
}

#[derive(Serialize)]
pub struct AccountPage {
    pub me: MeResponse,
    /// The admin panel or provider dashboard, when the visitor has one.
    pub area: Option<Link>,
    pub errors: FormErrors,
    pub password_changed: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PasswordForm {
    pub current_password: String,
    pub new_password: String,
    pub password2: String,
}

async fn page(
    state: &AppState,
    actor: &application::authz::Actor,
    errors: FormErrors,
    password_changed: bool,
) -> Result<AccountPage, Error> {
    let area = home_area(Some(actor));
    Ok(AccountPage {
        me: state.usecases.auth.me(actor.user_id).await?,
        area: (area.href != url("account")?).then_some(area),
        errors,
        password_changed,
    })
}

pub async fn show(
    shell: Shell,
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login")?);
    };
    let changed = flash.0 == Some(Flash::PasswordChanged);
    let page = page(&state, &actor, FormErrors::default(), changed).await?;
    let mut response = render_page(&state, &shell, "pages/account.html", &page)?;
    flash.clear_on(&mut response);
    Ok(response)
}

pub async fn change_password(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    Extension(policy): Extension<CookiePolicy>,
    ApiForm(form): ApiForm<PasswordForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login")?);
    };
    let auth = &state.usecases.auth;
    let result = match confirm_password(&form.new_password, &form.password2, "password2") {
        Err(e) => Err(e),
        Ok(()) => {
            auth.change_password(actor.user_id, form.current_password, form.new_password)
                .await
        }
    };
    match result {
        Ok(()) => {
            let tokens = auth.start_session(actor.user_id).await?;
            let mut response = see_other(&url("account_security")?)?;
            policy.set(&mut response, &tokens);
            Flash::PasswordChanged.set_on(&mut response);
            Ok(response)
        }
        Err(err) if is_form_error(&err) => {
            let errors = FormErrors::from_error(&state, &err);
            let page = page(&state, &actor, errors, false).await?;
            render_page_status(
                &state,
                &shell,
                err.kind().status(),
                "pages/account.html",
                &page,
            )
        }
        Err(err) => Err(err),
    }
}
