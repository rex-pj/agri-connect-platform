//! GET /account — who you are, and the security section — and
//! POST /account/password — change the password.
//!
//! Changing the password ends every session of the account (the use case
//! revokes them); this browser gets a fresh one so it stays signed in.
//!
//! The page sits in the frame of the visitor's own area, so the account menu's
//! "profile" never takes anyone out of where they work: the admin panel for
//! admins, the partner frame for partners, the app for everyone else.

use application::{authz::Actor, outputs::MeResponse};
use axum::{extract::State, http::StatusCode, response::Response, Extension};
use domain::Error;
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::ApiForm,
    labels::role_name,
    middleware::auth::Viewer,
    pages::{
        admin::render_admin,
        forms::{confirm_password, is_form_error, FormErrors},
        see_other,
    },
    routes,
    session::CookiePolicy,
    shell::{home_area, Area, Link, Shell},
    views::render_page_status,
};

fn url(name: &str) -> Result<String, Error> {
    routes::url(name, &[]).map_err(|e| Error::internal(format!("account links: {e}")))
}

#[derive(Serialize)]
pub struct AccountPage {
    pub me: MeResponse,
    /// The role, worded from its code.
    pub role_name: String,
    /// The provider dashboard or partner report, when the visitor has one.
    /// Not for admins: the panel's own menu is around the page.
    pub area: Option<Link>,
    pub errors: FormErrors,
    pub password_changed: bool,
    /// The confirmation email was just asked for again.
    pub verification_sent: bool,
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
    actor: &Actor,
    errors: FormErrors,
    flash: Option<Flash>,
) -> Result<AccountPage, Error> {
    let area = match Area::of(Some(actor)) {
        Area::Partner | Area::Provider => Some(home_area(state.locale(), Some(actor))),
        Area::Admin | Area::Visitor => None,
    };
    let me = state.usecases.auth.me(actor.user_id).await?;
    Ok(AccountPage {
        role_name: role_name(state.locale(), &me.role, &me.role),
        me,
        area,
        errors,
        password_changed: flash == Some(Flash::PasswordChanged),
        verification_sent: flash == Some(Flash::EmailVerificationSent),
    })
}

/// The page in the frame of the visitor's area.
fn render(
    state: &AppState,
    shell: &Shell,
    actor: &Actor,
    status: StatusCode,
    page: &AccountPage,
) -> Result<Response, Error> {
    match Area::of(Some(actor)) {
        Area::Admin => render_admin(
            state,
            actor,
            &shell.path,
            status,
            "pages/admin/account.html",
            page,
        ),
        Area::Partner => {
            render_page_status(state, shell, status, "pages/partner_account.html", page)
        }
        Area::Provider | Area::Visitor => {
            render_page_status(state, shell, status, "pages/account.html", page)
        }
    }
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
    let page = page(&state, &actor, FormErrors::default(), flash.0).await?;
    let mut response = render(&state, &shell, &actor, StatusCode::OK, &page)?;
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
            let page = page(&state, &actor, errors, None).await?;
            render(&state, &shell, &actor, err.kind().status(), &page)
        }
        Err(err) => Err(err),
    }
}

/// POST /account/email-verification — sends the confirmation link again.
pub async fn send_email_verification(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login")?);
    };
    state
        .usecases
        .auth
        .request_email_verification(actor.user_id)
        .await?;
    let mut response = see_other(&url("account")?)?;
    Flash::EmailVerificationSent.set_on(&mut response);
    Ok(response)
}
