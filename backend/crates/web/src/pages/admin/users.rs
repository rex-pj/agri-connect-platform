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
        PERM_USER_STATUS_UPDATE, PERM_USER_UPDATE,
    },
    outputs::{AdminPagedResponse, AdminUserDetailDto, AdminUserDto, PermissionDto, RoleDto},
    usecases::admin_usecase::HistoryOf,
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{
    models::admin_list::{AdminSort, SWITCH_LIST_STATES},
    Error, ErrorCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    labels::Worded,
    middleware::auth::Viewer,
    pages::{
        admin::{
            history_rows, is_refusal, render_admin, url, AdminTab, Badge, Confirm, HistoryRow,
            ListPosition, RecordAction, RecordHeader, Says, SearchBox, Toggle,
        },
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
    /// The on / off chips.
    pub states: Vec<AdminTab>,
}

/// The orders the account list offers besides its own (oldest account
/// first).
const USER_SORTS: &[AdminSort] = &[AdminSort::Newest, AdminSort::Name];

/// GET /admin/users
pub async fn list(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(position): ApiQuery<ListPosition>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let position = position.checked(SWITCH_LIST_STATES, USER_SORTS);
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = state
        .usecases
        .admin
        .list_users(
            &actor,
            position.page(),
            &position.filter(SWITCH_LIST_STATES),
        )
        .await?
        .worded(state.locale());
    let roles = state.usecases.admin.role_choices(&actor).await?;
    let locale = state.locale();
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
        search: position
            .search_box("admin_users")?
            .toggled(
                "gap",
                "unverified_email",
                locale.text("admin_toggle.unverified_email"),
            )
            .picked(
                "role_id",
                locale.text("admin_pick.role"),
                locale.text("admin_pick.all_roles"),
                roles.into_iter().map(|r| (r.id, r.name)).collect(),
            )
            .dated(state.locale())
            .sorted(locale, &position, "admin_sort.first_made", USER_SORTS),
        states: position.shared_state_chips(locale, "admin_users", SWITCH_LIST_STATES, None)?,
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
    /// Its name, state and actions (turn on; what it manages; turning it
    /// off in the menu), and the confirmation.
    pub header: RecordHeader,
    pub confirms: Vec<Confirm>,
    /// Its latest changes and sign-ins: who changed its role or switched it
    /// off, and why.
    pub history: Vec<HistoryRow>,
    pub permissions: Vec<PermissionDto>,
    /// Set when the visitor may change roles: the choices for the select.
    pub roles: Option<Vec<RoleDto>>,
    /// The user's role grid, when the visitor may read roles.
    pub role_href: Option<String>,
    pub role_action: String,
    /// The name and email form, when the visitor may change them (never on
    /// their own account: the account page asks for the password).
    pub edit: Option<UserEdit>,
    pub errors: FormErrors,
    pub flash: Option<Flash>,
}

/// The account's name and email as the form shows them: stored, or as typed
/// after a refusal with its messages.
#[derive(Serialize)]
pub struct UserEdit {
    pub action: String,
    pub full_name: String,
    pub new_email: String,
    pub errors: FormErrors,
}

/// POST /admin/users/{id}/profile: the name, and an address to move the
/// account to (its current one: no move), with an optional reason.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct EditForm {
    pub full_name: String,
    pub new_email: String,
    pub reason: String,
}

/// What a refused form brings back to the page.
#[derive(Default)]
struct Refused {
    errors: FormErrors,
    /// A confirmation to show open again.
    confirm_open: bool,
    /// The name and email as typed: the messages are that form's.
    edit: Option<EditForm>,
}

/// The confirmation that sends the account a password-reset link.
const RESET_SHEET: &str = "gui-dat-lai-mat-khau";

/// The account's header: turning it back on is a button, switching it off
/// (it signs out at once) waits in the menu; the business or partner it
/// manages is a link beside them.
fn user_controls(
    state: &AppState,
    actor: &application::authz::Actor,
    user: &AdminUserDetailDto,
    business_href: Option<String>,
    partner_href: Option<String>,
    confirm_open: bool,
) -> Result<(RecordHeader, Vec<Confirm>), Error> {
    let locale = state.locale();
    let mut actions = Vec::new();
    let mut menu = Vec::new();
    let mut confirms = Vec::new();
    if actor.can(PERM_USER_STATUS_UPDATE) {
        let (toggle, off) = Toggle {
            is_on: user.is_active,
            action: url("admin_user_status", &[("id", &user.id.to_string())])?,
            field: "is_active",
            sheet: "tat-tai-khoan",
            off_label: locale.text("admin.turn_off_account"),
            off_icon: "fa-user-slash",
            on_label: locale.text("admin.turn_on_account"),
            on_icon: "fa-user-check",
            title: locale.text_with("admin.turn_off_account_title", &[("name", &user.name)]),
            button: locale.text("admin.confirm_off"),
            says: Says::Note(locale.text("admin.turn_off_account_note")),
        }
        .split();
        match off {
            Some(confirm) => {
                menu.push(toggle);
                confirms.push(confirm.opened(confirm_open));
            }
            None => actions.push(toggle),
        }
    }
    if actor.can(PERM_USER_UPDATE) && actor.user_id != user.id && user.is_active {
        menu.push(RecordAction::sheet(
            locale.text("admin.send_reset_link"),
            "fa-key",
            RESET_SHEET,
            "secondary",
        ));
        confirms.push(
            Confirm {
                button_class: "btn-success",
                hidden_name: "",
                hidden_value: "",
                ..Confirm::off(
                    RESET_SHEET,
                    locale.text_with("admin.send_reset_link_title", &[("email", &user.email)]),
                    "fa-key",
                    url("admin_user_password_reset", &[("id", &user.id.to_string())])?,
                    locale.text("admin.send_reset_link"),
                    "",
                )
            }
            .noting(locale.text("admin.send_reset_link_note")),
        );
    }
    if let Some(href) = business_href {
        actions.push(RecordAction::link(
            locale.text("admin.open_business"),
            "fa-store",
            href,
        ));
    }
    if let Some(href) = partner_href {
        actions.push(RecordAction::link(
            locale.text("admin.open_partner"),
            "fa-handshake",
            href,
        ));
    }
    let header = RecordHeader {
        title: user.name.clone(),
        icon: "fa-user".to_owned(),
        sub: Some(user.email.clone()),
        back_href: url("admin_users", &[])?,
        back_label: locale.text("admin_menu.users"),
        badges: vec![Badge::new("account", user.is_active)],
        actions,
        menu,
    };
    Ok((header, confirms))
}

async fn user_page(
    state: &AppState,
    actor: &application::authz::Actor,
    id: i32,
    refused: Refused,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response, Error> {
    let Refused {
        errors,
        confirm_open,
        edit: typed,
    } = refused;
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
    let (header, confirms) = user_controls(
        state,
        actor,
        &user,
        business_href,
        partner_href,
        confirm_open,
    )?;
    // The refusal's messages belong to the form that was sent.
    let (edit_errors, errors) = match typed {
        Some(_) => (errors, FormErrors::default()),
        None => (FormErrors::default(), errors),
    };
    let edit = if actor.can(PERM_USER_UPDATE) && actor.user_id != id {
        let (full_name, new_email) = match typed {
            Some(form) => (form.full_name, form.new_email),
            None => (
                user.full_name.clone().unwrap_or_default(),
                user.email.clone(),
            ),
        };
        Some(UserEdit {
            action: url("admin_user_profile", &[("id", &id_text)])?,
            full_name,
            new_email,
            errors: edit_errors,
        })
    } else {
        None
    };
    let body = UserPage {
        edit,
        header,
        confirms,
        history: history_rows(state, actor, HistoryOf::User(id)).await?,
        permissions,
        roles,
        role_href,
        role_action: url("admin_user_role", &[("id", &id_text)])?,
        user,
        errors,
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
        Refused::default(),
        flash.0,
        StatusCode::OK,
    )
    .await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// After a change: back to the user with a flash, or the page re-shown with
/// the refusal (own account, invalid role, a broken box), `refused` saying
/// which form it goes with.
async fn after<T>(
    state: &AppState,
    actor: &application::authz::Actor,
    id: i32,
    result: Result<T, Error>,
    flash: Flash,
    refused: impl FnOnce(FormErrors) -> Refused,
) -> Result<Response, Error> {
    match result {
        Ok(_) => {
            let mut response = see_other(&url("admin_user", &[("id", &id.to_string())])?)?;
            flash.set_on(&mut response);
            Ok(response)
        }
        Err(err) if is_refusal(&err) => {
            let errors = FormErrors::from_error(state, &err);
            user_page(state, actor, id, refused(errors), None, err.kind().status()).await
        }
        Err(err) => Err(err),
    }
}

/// A refusal shown above the page, with confirmation `open` again.
fn on_page(open: bool) -> impl FnOnce(FormErrors) -> Refused {
    move |errors| Refused {
        errors,
        confirm_open: open,
        edit: None,
    }
}

/// POST /admin/users/{id}/profile — the account's name, and the address to
/// move it to when the box holds another one (a link is sent there: the
/// account keeps its address until it is opened).
pub async fn save_profile(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<EditForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::UserNotFound)?;
    let result = state
        .usecases
        .auth
        .admin_update_user(
            &actor,
            id,
            Some(form.full_name.clone()),
            Some(form.new_email.clone()),
            reason(&form.reason),
        )
        .await;
    let flash = match result {
        Ok(true) => Flash::EmailChangeSent,
        _ => Flash::UserSaved,
    };
    after(&state, &actor, id, result, flash, |errors| Refused {
        errors,
        confirm_open: false,
        edit: Some(form),
    })
    .await
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReasonForm {
    pub reason: String,
}

/// POST /admin/users/{id}/password-reset — a reset link to the account's
/// address (behind its confirmation).
pub async fn send_password_reset(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<ReasonForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::UserNotFound)?;
    let result = state
        .usecases
        .auth
        .admin_send_password_reset(&actor, id, reason(&form.reason))
        .await;
    after(
        &state,
        &actor,
        id,
        result,
        Flash::PasswordResetSent,
        on_page(true),
    )
    .await
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
    after(
        &state,
        &actor,
        id,
        result,
        Flash::UserRoleChanged,
        on_page(false),
    )
    .await
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
    after(&state, &actor, id, result, flash, on_page(!activate)).await
}
