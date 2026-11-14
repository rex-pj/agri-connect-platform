//! /admin/users and /admin/users/{id}: the account list, and one account's
//! role, status and effective permissions.
//!
//! Deactivating asks for confirmation and a reason (it signs the user out at
//! once); with JS it is a dialog, without JS the same form on the page.
//! Reactivating is one button. An admin cannot change their own role or
//! status (`cannot_change_own_account`, from the use case).

use application::{
    catalogue::{
        PERM_PARTNER_MANAGE, PERM_PROVIDER_READ, PERM_ROLE_READ, PERM_USER_ROLE_UPDATE,
        PERM_USER_STATUS_UPDATE,
    },
    outputs::{AdminPagedResponse, AdminUserDetailDto, AdminUserDto, PermissionDto, RoleDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    labels::Worded,
    middleware::auth::Viewer,
    pages::{
        admin::{is_refusal, render_admin, url, ListPosition, SearchBox},
        forms::{reason, FormErrors},
        listing::Pagination,
        parse_id, see_other,
    },
};

#[derive(Serialize)]
pub struct UserRow {
    pub user: AdminUserDto,
    pub href: String,
}

#[derive(Serialize)]
pub struct UsersPage {
    pub total: i64,
    pub rows: Vec<UserRow>,
    pub pagination: Option<Pagination>,
    pub search: SearchBox,
}

/// GET /admin/users
pub async fn list(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(position): ApiQuery<ListPosition>,
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
        .list_users(&actor, position.page(), position.search())
        .await?
        .worded(state.locale());
    let path = url("admin_users", &[])?;
    let mut rows = Vec::with_capacity(items.len());
    for user in items {
        let href = url("admin_user", &[("id", &user.id.to_string())])?;
        rows.push(UserRow { user, href });
    }
    let body = UsersPage {
        total,
        rows,
        pagination: position.pagination(&path, page, total_pages),
        search: position.search_box("admin_users")?,
    };
    render_admin(
        &state,
        &actor,
        &path,
        StatusCode::OK,
        "pages/admin/users.html",
        &body,
    )
}

#[derive(Serialize)]
pub struct UserPage {
    pub user: AdminUserDetailDto,
    pub permissions: Vec<PermissionDto>,
    /// Set when the visitor may change roles: the choices for the select.
    pub roles: Option<Vec<RoleDto>>,
    pub can_change_status: bool,
    /// The user's role grid, when the visitor may read roles.
    pub role_href: Option<String>,
    /// The business this account manages, when the visitor may open it.
    pub business_href: Option<String>,
    /// The partner whose report account this is, likewise.
    pub partner_href: Option<String>,
    pub role_action: String,
    pub status_action: String,
    pub errors: FormErrors,
    /// The dialog re-opens after a refused deactivation.
    pub confirm_open: bool,
    pub flash: Option<Flash>,
}

async fn user_page(
    state: &AppState,
    actor: &application::authz::Actor,
    id: i32,
    errors: FormErrors,
    confirm_open: bool,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response, Error> {
    let admin = &state.usecases.admin;
    let user = admin
        .get_user_detail(actor, id)
        .await?
        .worded(state.locale());
    let permissions = state
        .usecases
        .permission
        .list_by_user(actor, id)
        .await?
        .worded(state.locale())
        .permissions;
    let roles = if actor.can(PERM_USER_ROLE_UPDATE) {
        Some(admin.assignable_roles(actor).await?.worded(state.locale()))
    } else {
        None
    };
    let role_href = if actor.can(PERM_ROLE_READ) {
        admin
            .list_roles(actor)
            .await?
            .into_iter()
            .find(|r| r.code == user.role_code)
            .map(|r| url("admin_role", &[("id", &r.id.to_string())]))
            .transpose()?
    } else {
        None
    };
    // What the account manages, each linked when the visitor may open it.
    let business_href = match user.business_id {
        Some(b) if actor.can(PERM_PROVIDER_READ) => {
            Some(url("admin_provider", &[("id", &b.to_string())])?)
        }
        _ => None,
    };
    let partner_href = if actor.can(PERM_PARTNER_MANAGE) {
        state
            .usecases
            .partner
            .partner_of_account(actor, id)
            .await?
            .map(|p| url("admin_partner", &[("id", &p.to_string())]))
            .transpose()?
    } else {
        None
    };
    let id_text = id.to_string();
    let body = UserPage {
        permissions,
        roles,
        can_change_status: actor.can(PERM_USER_STATUS_UPDATE),
        role_href,
        business_href,
        partner_href,
        role_action: url("admin_user_role", &[("id", &id_text)])?,
        status_action: url("admin_user_status", &[("id", &id_text)])?,
        user,
        errors,
        confirm_open,
        flash,
    };
    render_admin(
        state,
        actor,
        &url("admin_user", &[("id", &id_text)])?,
        status,
        "pages/admin/user.html",
        &body,
    )
}

/// GET /admin/users/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::UserNotFound)?;
    let mut response = user_page(
        &state,
        &actor,
        id,
        FormErrors::default(),
        false,
        flash.0,
        StatusCode::OK,
    )
    .await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// After a change: back to the user with a flash, or the page re-shown with
/// the refusal (own account, invalid role).
async fn after(
    state: &AppState,
    actor: &application::authz::Actor,
    id: i32,
    result: Result<AdminUserDetailDto, Error>,
    flash: Flash,
    confirm_open: bool,
) -> Result<Response, Error> {
    match result {
        Ok(_) => {
            let mut response = see_other(&url("admin_user", &[("id", &id.to_string())])?)?;
            flash.set_on(&mut response);
            Ok(response)
        }
        Err(err) if is_refusal(&err) => {
            let errors = FormErrors::from_error(state, &err);
            user_page(
                state,
                actor,
                id,
                errors,
                confirm_open,
                None,
                err.kind().status(),
            )
            .await
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RoleForm {
    pub role_code: String,
}

/// POST /admin/users/{id}/role
pub async fn change_role(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<RoleForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::UserNotFound)?;
    let result = state
        .usecases
        .admin
        .update_user_role(&actor, id, form.role_code)
        .await;
    after(&state, &actor, id, result, Flash::UserRoleChanged, false).await
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    /// "true" to activate, "false" to deactivate.
    pub is_active: String,
    pub reason: String,
}

/// POST /admin/users/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::UserNotFound)?;
    let activate = form.is_active == "true";
    let reason = reason(&form.reason);
    let result = state
        .usecases
        .admin
        .toggle_user_active(&actor, id, activate, reason)
        .await;
    let flash = if activate {
        Flash::UserActivated
    } else {
        Flash::UserDeactivated
    };
    after(&state, &actor, id, result, flash, !activate).await
}
