//! GET /account — who you are and what you may change about it:
//! - POST /account/profile — the name the account is shown by;
//! - POST /account/email — move to another address (the current password,
//!   then the link sent there: nothing changes until it is opened);
//! - POST /account/email-verification — the confirmation link again;
//! - POST /account/password — change the password;
//! - POST /account/sessions — sign out every other device.
//!
//! Changing the password or signing out the other devices ends every
//! session of the account (the use case revokes them); this browser gets a
//! fresh one so it stays signed in.
//!
//! The page sits in the frame of the visitor's own area, so the account menu's
//! "profile" never takes anyone out of where they work: the admin panel for
//! admins, the partner frame for partners, the app for everyone else.

use application::{authz::Actor, outputs::MeResponse};
use axum::{extract::State, http::StatusCode, response::Response, Extension};
use domain::{Error, ErrorKind};
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

/// Which of the page's forms came back refused, with what was typed.
#[derive(Default)]
enum Refused {
    #[default]
    None,
    Profile(ProfileForm, FormErrors),
    Email(EmailForm, FormErrors),
    Password(FormErrors),
}

#[derive(Serialize)]
pub struct AccountPage {
    pub me: MeResponse,
    /// The role, worded from its code.
    pub role_name: String,
    /// The provider dashboard or partner report, when the visitor has one.
    /// Not for admins: the panel's own menu is around the page.
    pub area: Option<Link>,
    /// The name box: as typed after a refusal, else the stored name.
    pub full_name: String,
    pub profile_errors: FormErrors,
    /// The "change email" sheet's address as typed after a refusal.
    pub new_email: String,
    pub email_errors: FormErrors,
    /// The password form's errors.
    pub errors: FormErrors,
    pub profile_saved: bool,
    /// The link to the new address was just sent.
    pub email_change_sent: bool,
    pub password_changed: bool,
    pub sessions_ended: bool,
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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ProfileForm {
    pub full_name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct EmailForm {
    pub new_email: String,
    pub current_password: String,
}

async fn page(
    state: &AppState,
    actor: &Actor,
    refused: Refused,
    flash: Option<Flash>,
) -> Result<AccountPage, Error> {
    let area = match Area::of(Some(actor)) {
        Area::Partner | Area::Provider => Some(home_area(state.locale(), Some(actor))),
        Area::Admin | Area::Visitor => None,
    };
    let me = state.usecases.auth.me(actor.user_id).await?;
    let mut page = AccountPage {
        role_name: role_name(state.locale(), &me.role, &me.role),
        full_name: me.full_name.clone().unwrap_or_default(),
        me,
        area,
        profile_errors: FormErrors::default(),
        new_email: String::new(),
        email_errors: FormErrors::default(),
        errors: FormErrors::default(),
        profile_saved: flash == Some(Flash::ProfileSaved),
        email_change_sent: flash == Some(Flash::EmailChangeSent),
        password_changed: flash == Some(Flash::PasswordChanged),
        sessions_ended: flash == Some(Flash::OtherSessionsEnded),
        verification_sent: flash == Some(Flash::EmailVerificationSent),
    };
    match refused {
        Refused::None => {}
        Refused::Profile(form, errors) => {
            page.full_name = form.full_name;
            page.profile_errors = errors;
        }
        Refused::Email(form, errors) => {
            page.new_email = form.new_email;
            page.email_errors = errors;
        }
        Refused::Password(errors) => page.errors = errors,
    }
    Ok(page)
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

/// A refusal the form can show (a broken rule, or too many wrong current
/// passwords, said above the form): the page again with the error's status.
/// Anything else is the error page.
async fn reshow(
    state: &AppState,
    shell: &Shell,
    actor: &Actor,
    err: Error,
    refused: impl FnOnce(FormErrors) -> Refused,
) -> Result<Response, Error> {
    if !(is_form_error(&err) || err.kind() == ErrorKind::TooManyRequests) {
        return Err(err);
    }
    let errors = FormErrors::from_error(state, &err);
    let page = page(state, actor, refused(errors), None).await?;
    render(state, shell, actor, err.kind().status(), &page)
}

/// Back to the page (`to`: its route name), with `flash` to say what was done.
fn done(to: &str, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url(to)?)?;
    flash.set_on(&mut response);
    Ok(response)
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
    let page = page(&state, &actor, Refused::None, flash.0).await?;
    let mut response = render(&state, &shell, &actor, StatusCode::OK, &page)?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /account/profile — the name the account is shown by.
pub async fn save_profile(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<ProfileForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login")?);
    };
    match state
        .usecases
        .auth
        .update_profile(actor.user_id, Some(form.full_name.clone()))
        .await
    {
        Ok(()) => done("account", Flash::ProfileSaved),
        Err(err) => reshow(&state, &shell, &actor, err, |e| Refused::Profile(form, e)).await,
    }
}

/// POST /account/email — asks to move the account to another address.
pub async fn change_email(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<EmailForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login")?);
    };
    match state
        .usecases
        .auth
        .request_email_change(
            actor.user_id,
            form.current_password.clone(),
            form.new_email.clone(),
        )
        .await
    {
        Ok(()) => done("account_email_box", Flash::EmailChangeSent),
        Err(err) => reshow(&state, &shell, &actor, err, |e| Refused::Email(form, e)).await,
    }
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
            let mut response = done("account_security", Flash::PasswordChanged)?;
            policy.set(&mut response, &tokens);
            Ok(response)
        }
        Err(err) => reshow(&state, &shell, &actor, err, Refused::Password).await,
    }
}

/// POST /account/sessions — every other device of the account is signed
/// out; this browser starts a new session.
pub async fn end_other_sessions(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    Extension(policy): Extension<CookiePolicy>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login")?);
    };
    let auth = &state.usecases.auth;
    auth.end_other_sessions(actor.user_id).await?;
    let tokens = auth.start_session(actor.user_id).await?;
    let mut response = done("account_security", Flash::OtherSessionsEnded)?;
    policy.set(&mut response, &tokens);
    Ok(response)
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
    done("account_email_box", Flash::EmailVerificationSent)
}
