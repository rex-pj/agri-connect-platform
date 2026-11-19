//! /admin/providers and /admin/providers/{id}: the provider list with a create
//! form, and one provider's details, profile, verification badge and
//! managing account.
//!
//! The create form is a sheet on the list: an admin on a field visit lists a
//! business from its owner's phone number, with no account (one can be
//! assigned later on its page), and lands on its page to add its services.
//! Its page carries the same profile form its owner has on the dashboard.
//!
//! Withdrawing the badge asks for confirmation and a reason (a dialog with
//! JS, the same form on the page without); granting it is one button. Both
//! set an explicit state, so a form sent twice changes nothing. The account
//! form links an existing provider account by username, or creates one.
//! Turning a provider off (gone from every public page with its listings)
//! also asks for confirmation and a reason; turning it on is one button.

use application::{
    authz::Actor,
    catalogue::{
        PERM_INQUIRY_READ, PERM_LISTING_MODERATE, PERM_PROVIDER_ACCOUNT_ASSIGN,
        PERM_PROVIDER_MANAGE, PERM_PROVIDER_STATUS_UPDATE, PERM_PROVIDER_VERIFY,
        PERM_REVIEW_MODERATE, PERM_SERVICE_MANAGE,
    },
    outputs::{AdminPagedResponse, AdminProviderDetailDto, AdminProviderDto, ProviderActivityDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::MAP_POINT_URL,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{
            dashboard::{card, StatCard},
            is_refusal, render_admin, url, user_href, ListPosition, Outcome, SearchBox,
        },
        canonical_url,
        forms::{reason, FormErrors},
        listing::Pagination,
        parse_id,
        profile_form::ProfileForm,
        see_other,
    },
};

#[derive(Serialize)]
pub struct ProviderRow {
    pub provider: AdminProviderDto,
    pub href: String,
}

#[derive(Serialize)]
pub struct ProvidersPage {
    pub total: i64,
    pub rows: Vec<ProviderRow>,
    pub pagination: Option<Pagination>,
    pub search: SearchBox,
    /// The create form, `None` without `provider.manage`.
    pub create: Option<CreateSheet>,
}

/// The create sheet, with the values as typed after a refusal.
#[derive(Serialize)]
pub struct CreateSheet {
    pub action: String,
    pub form: ProfileForm,
    pub errors: FormErrors,
}

async fn list_page(
    state: &AppState,
    actor: &Actor,
    position: &ListPosition,
    typed: Option<(ProfileForm, FormErrors)>,
    status: StatusCode,
) -> Result<Response, Error> {
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = state
        .usecases
        .admin
        .list_providers(actor, position.page(), position.search())
        .await?;
    let path = url("admin_providers", &[])?;
    let mut rows = Vec::with_capacity(items.len());
    for provider in items {
        let href = url("admin_provider", &[("id", &provider.id.to_string())])?;
        rows.push(ProviderRow { provider, href });
    }
    let create = actor.can(PERM_PROVIDER_MANAGE).then(|| {
        let (form, errors) = typed.unwrap_or_default();
        CreateSheet {
            action: path.clone(),
            form,
            errors,
        }
    });
    let body = ProvidersPage {
        total,
        rows,
        pagination: position.pagination(&path, page, total_pages),
        search: position.search_box("admin_providers")?,
        create,
    };
    render_admin(
        state,
        actor,
        &path,
        status,
        "pages/admin/providers.html",
        &body,
    )
}

/// GET /admin/providers
pub async fn list(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(position): ApiQuery<ListPosition>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    list_page(&state, &actor, &position, None, StatusCode::OK).await
}

/// POST /admin/providers
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<ProfileForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    match state
        .usecases
        .admin
        .create_provider(&actor, form.input())
        .await
    {
        Ok(id) => redirect_to(id, Flash::ProviderCreated),
        Err(err) if is_refusal(&err) => {
            let typed = (form, FormErrors::from_error(&state, &err));
            let status = err.kind().status();
            list_page(
                &state,
                &actor,
                &ListPosition::default(),
                Some(typed),
                status,
            )
            .await
        }
        Err(err) => Err(err),
    }
}

/// The account form as typed. The password is never sent back.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AccountForm {
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password: String,
}

/// The form of the page that came back refused, with its values as typed.
pub enum Typed {
    Profile(ProfileForm),
    Account(AccountForm),
}

/// What farmers did about the provider lately, as the dashboard's cards:
/// taps, contact requests (to their list), reviews (to theirs); a figure
/// the visitor may not read is left out.
fn activity_cards(
    state: &AppState,
    activity: &ProviderActivityDto,
    inquiries_href: Option<&String>,
    reviews_href: Option<&String>,
) -> Vec<StatCard> {
    let locale = state.locale();
    let mut cards = vec![card(
        locale.text_with("admin_stats.calls", &[("days", &activity.days)]),
        activity.taps,
        "fa-phone",
        "green",
    )];
    if let Some(i) = &activity.inquiries {
        let note = if i.waiting > 0 {
            Some(locale.text_with("admin.inquiries_waiting", &[("count", &i.waiting)]))
        } else {
            (i.total > 0).then(|| locale.text("admin.inquiries_none_waiting"))
        };
        cards.push(StatCard {
            href: inquiries_href.cloned(),
            note,
            ..card(
                locale.text("contact.title"),
                i.total,
                "fa-envelope-open-text",
                "blue",
            )
        });
    }
    if let Some(r) = &activity.reviews {
        let mut parts = Vec::new();
        if let Some(average) = r.average {
            parts.push(locale.text_with("admin.reviews_average", &[("average", &average)]));
        }
        if r.waiting > 0 {
            parts.push(locale.text_with("admin.reviews_waiting", &[("count", &r.waiting)]));
        }
        cards.push(StatCard {
            href: reviews_href.cloned(),
            note: (!parts.is_empty()).then(|| parts.join(" · ")),
            ..card(
                locale.text("admin_menu.reviews"),
                r.shown,
                "fa-star",
                "amber",
            )
        });
    }
    cards
}

#[derive(Serialize)]
pub struct ProviderPage {
    pub provider: AdminProviderDetailDto,
    /// Its recent activity, as the dashboard's cards.
    pub cards: Vec<StatCard>,
    /// Its position on a map, to check where it was placed.
    pub map_href: Option<String>,
    /// The public page, to see what farmers see.
    pub public_href: String,
    /// The managing account's admin page, when the visitor may read accounts.
    pub linked_user_href: Option<String>,
    pub can_verify: bool,
    pub can_assign: bool,
    pub can_change_status: bool,
    pub can_edit: bool,
    pub verification_action: String,
    pub status_action: String,
    pub account_action: String,
    pub profile_action: String,
    /// This provider's services and products in their admin lists, when the
    /// visitor may manage them.
    pub services_href: Option<String>,
    pub products_href: Option<String>,
    /// The farmers' inquiries and reviews about it, likewise.
    pub inquiries_href: Option<String>,
    pub reviews_href: Option<String>,
    pub form: AccountForm,
    pub profile: ProfileForm,
    /// The profile form's messages, apart so they never show in the others.
    pub profile_errors: FormErrors,
    pub errors: FormErrors,
    /// The form `errors` belong to: "account", or "" (a refused confirmation,
    /// shown above everything); "profile" when they are in `profile_errors`.
    pub failed: &'static str,
    /// The unverify dialog re-opens after a refusal.
    pub confirm_open: bool,
    pub flash: Option<Flash>,
}

async fn provider_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<Typed>,
) -> Result<Response, Error> {
    let provider = state.usecases.admin.get_provider_detail(actor, id).await?;
    let id_text = id.to_string();
    let path = url("admin_provider", &[("id", &id_text)])?;
    let (typed_profile, typed_account, failed) = match outcome.form {
        Some(Typed::Profile(f)) => (Some(f), None, "profile"),
        Some(Typed::Account(f)) => (None, Some(f), "account"),
        None => (None, None, ""),
    };
    // A business without an account is usually set up with its phone number
    // as the username, which is what the owner remembers.
    let form = typed_account.unwrap_or_else(|| AccountForm {
        username: provider
            .linked_username
            .clone()
            .unwrap_or_else(|| provider.phone.clone()),
        ..AccountForm::default()
    });
    let (profile, profile_errors, errors) = match typed_profile {
        Some(typed) => (typed, outcome.errors, FormErrors::default()),
        None => (
            ProfileForm::from_admin(&provider),
            FormErrors::default(),
            outcome.errors,
        ),
    };
    let own = ListPosition {
        provider_id: id_text.clone(),
        ..ListPosition::default()
    };
    let list_of = |perm: &str, route: &str| -> Result<Option<String>, Error> {
        actor.can(perm).then(|| own.href(route, &[])).transpose()
    };
    let inquiries_href = list_of(PERM_INQUIRY_READ, "admin_inquiries")?;
    let reviews_href = list_of(PERM_REVIEW_MODERATE, "admin_reviews")?;
    let body = ProviderPage {
        cards: activity_cards(
            state,
            &provider.activity,
            inquiries_href.as_ref(),
            reviews_href.as_ref(),
        ),
        map_href: provider
            .lat
            .zip(provider.lng)
            .map(|(lat, lng)| format!("{MAP_POINT_URL}{lat},{lng}")),
        services_href: list_of(PERM_SERVICE_MANAGE, "admin_services")?,
        products_href: list_of(PERM_LISTING_MODERATE, "admin_products")?,
        inquiries_href,
        reviews_href,
        public_href: canonical_url("provider", id, &provider.name)?,
        linked_user_href: user_href(actor, provider.linked_user_id)?,
        provider,
        can_verify: actor.can(PERM_PROVIDER_VERIFY),
        can_assign: actor.can(PERM_PROVIDER_ACCOUNT_ASSIGN),
        can_change_status: actor.can(PERM_PROVIDER_STATUS_UPDATE),
        can_edit: actor.can(PERM_PROVIDER_MANAGE),
        verification_action: url("admin_provider_verification", &[("id", &id_text)])?,
        status_action: url("admin_provider_status", &[("id", &id_text)])?,
        account_action: url("admin_provider_account", &[("id", &id_text)])?,
        profile_action: path.clone(),
        form,
        profile,
        profile_errors,
        errors,
        failed,
        confirm_open: outcome.confirm_open,
        flash: outcome.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/provider.html",
        &body,
    )
}

/// The page again after one of its forms was refused.
async fn refused(
    state: &AppState,
    actor: &Actor,
    id: i32,
    err: &Error,
    typed: Typed,
) -> Result<Response, Error> {
    let outcome = Outcome::refused(state, err, Some(typed), false);
    provider_page(state, actor, id, outcome).await
}

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_provider", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// GET /admin/providers/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProviderNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let mut response = provider_page(&state, &actor, id, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/providers/{id}: the profile form.
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<ProfileForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProviderNotFound)?;
    match state
        .usecases
        .admin
        .update_provider(&actor, id, form.input())
        .await
    {
        Ok(()) => redirect_to(id, Flash::ProviderSaved),
        Err(err) if is_refusal(&err) => {
            refused(&state, &actor, id, &err, Typed::Profile(form)).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct VerificationForm {
    /// "true" to grant the badge, "false" to withdraw it.
    pub is_verified: String,
    pub reason: String,
}

/// POST /admin/providers/{id}/verification
pub async fn set_verification(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<VerificationForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProviderNotFound)?;
    let verified = form.is_verified == "true";
    state
        .usecases
        .admin
        .set_verification(&actor, id, verified, reason(&form.reason))
        .await?;
    let flash = if verified {
        Flash::ProviderVerified
    } else {
        Flash::ProviderUnverified
    };
    redirect_to(id, flash)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    /// "true" to turn the provider on, "false" to turn it off.
    pub is_active: String,
    pub reason: String,
}

/// POST /admin/providers/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProviderNotFound)?;
    let active = form.is_active == "true";
    state
        .usecases
        .admin
        .toggle_provider_active(&actor, id, active, reason(&form.reason))
        .await?;
    let flash = if active {
        Flash::ProviderActivated
    } else {
        Flash::ProviderDeactivated
    };
    redirect_to(id, flash)
}

/// POST /admin/providers/{id}/account
pub async fn assign_account(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<AccountForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProviderNotFound)?;
    let result = state
        .usecases
        .admin
        .assign_provider_user(
            &actor,
            id,
            form.username.clone(),
            form.email.clone(),
            form.password.clone(),
        )
        .await;
    match result {
        Ok(_) => redirect_to(id, Flash::ProviderAccountAssigned),
        Err(err) if is_refusal(&err) => {
            refused(&state, &actor, id, &err, Typed::Account(form)).await
        }
        Err(err) => Err(err),
    }
}
