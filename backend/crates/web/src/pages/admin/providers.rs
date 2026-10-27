//! /admin/explore and /admin/explore/{id}: the provider list, and one
//! provider's details, verification badge and managing account.
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
        PERM_PROVIDER_STATUS_UPDATE, PERM_PROVIDER_VERIFY, PERM_REVIEW_MODERATE,
        PERM_SERVICE_MANAGE,
    },
    outputs::{AdminPagedResponse, AdminProviderDetailDto, AdminProviderDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{is_refusal, render_admin, url, ListPosition, Outcome},
        canonical_url,
        forms::{reason, FormErrors},
        listing::{parse_opt, Pagination},
        parse_id, see_other,
    },
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
}

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
}

/// GET /admin/explore
pub async fn list(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = state
        .usecases
        .admin
        .list_providers(&actor, parse_opt(&q.page).unwrap_or(1))
        .await?;
    let path = url("admin_explore", &[])?;
    let mut rows = Vec::with_capacity(items.len());
    for provider in items {
        let href = url("admin_provider", &[("id", &provider.id.to_string())])?;
        rows.push(ProviderRow { provider, href });
    }
    let body = ProvidersPage {
        total,
        rows,
        pagination: Pagination::new(&path, &[], page, total_pages),
    };
    render_admin(
        &state,
        &actor,
        &path,
        StatusCode::OK,
        "pages/admin/providers.html",
        &body,
    )
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

#[derive(Serialize)]
pub struct ProviderPage {
    pub provider: AdminProviderDetailDto,
    /// The public page, to see what farmers see.
    pub public_href: String,
    pub can_verify: bool,
    pub can_assign: bool,
    pub can_change_status: bool,
    pub verification_action: String,
    pub status_action: String,
    pub account_action: String,
    /// This provider's services and products in their admin lists, when the
    /// visitor may manage them.
    pub services_href: Option<String>,
    pub products_href: Option<String>,
    /// The farmers' inquiries and reviews about it, likewise.
    pub inquiries_href: Option<String>,
    pub reviews_href: Option<String>,
    pub form: AccountForm,
    pub errors: FormErrors,
    /// The unverify dialog re-opens after a refusal.
    pub confirm_open: bool,
    pub flash: Option<Flash>,
}

async fn provider_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<AccountForm>,
) -> Result<Response, Error> {
    let provider = state.usecases.admin.get_provider_detail(actor, id).await?;
    let id_text = id.to_string();
    let path = url("admin_provider", &[("id", &id_text)])?;
    // A business without an account is usually set up with its phone number
    // as the username, which is what the owner remembers.
    let form = outcome.form.unwrap_or_else(|| AccountForm {
        username: provider
            .linked_username
            .clone()
            .unwrap_or_else(|| provider.phone.clone()),
        ..AccountForm::default()
    });
    let own = ListPosition {
        provider_id: id_text.clone(),
        ..ListPosition::default()
    };
    let list_of = |perm: &str, route: &str| -> Result<Option<String>, Error> {
        actor.can(perm).then(|| own.href(route, &[])).transpose()
    };
    let body = ProviderPage {
        services_href: list_of(PERM_SERVICE_MANAGE, "admin_services")?,
        products_href: list_of(PERM_LISTING_MODERATE, "admin_products")?,
        inquiries_href: list_of(PERM_INQUIRY_READ, "admin_inquiries")?,
        reviews_href: list_of(PERM_REVIEW_MODERATE, "admin_reviews")?,
        public_href: canonical_url("provider", id, &provider.name)?,
        provider,
        can_verify: actor.can(PERM_PROVIDER_VERIFY),
        can_assign: actor.can(PERM_PROVIDER_ACCOUNT_ASSIGN),
        can_change_status: actor.can(PERM_PROVIDER_STATUS_UPDATE),
        verification_action: url("admin_provider_verification", &[("id", &id_text)])?,
        status_action: url("admin_provider_status", &[("id", &id_text)])?,
        account_action: url("admin_provider_account", &[("id", &id_text)])?,
        form,
        errors: outcome.errors,
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

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_provider", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// GET /admin/explore/{id}
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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct VerificationForm {
    /// "true" to grant the badge, "false" to withdraw it.
    pub is_verified: String,
    pub reason: String,
}

/// POST /admin/explore/{id}/verification
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

/// POST /admin/explore/{id}/status
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

/// POST /admin/explore/{id}/account
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
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            provider_page(&state, &actor, id, outcome).await
        }
        Err(err) => Err(err),
    }
}
