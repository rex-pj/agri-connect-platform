//! /admin/roles, /admin/roles/{id} (with the editable permission grid) and
//! /admin/permissions (permissions × roles).
//!
//! The grid posts one checkbox per permission, named `perm.<key>`: an
//! urlencoded form cannot repeat a field name into a list here, and named
//! boxes keep the form working without JS.

use std::collections::HashMap;

use application::{
    catalogue::PERM_ROLE_PERMISSION_MANAGE,
    outputs::{GrantDto, PermissionMatrixDto, RoleDto, RoleGrantsDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    labels::Worded,
    locale::Locale,
    middleware::auth::Viewer,
    pages::{
        admin::{render_admin, url, AdminTab, ListPosition},
        forms::{is_form_error, FormErrors},
        moved_permanently, parse_id, see_other,
    },
};

/// The checkbox name prefix of the grid.
const FIELD_PREFIX: &str = "perm.";

/// The text key of a permission's group heading, from the first part of its
/// key.
fn group_key(key: &str) -> &'static str {
    match key.split('.').next().unwrap_or_default() {
        "admin" | "system" | "setting" => "permission_group.system",
        "user" | "role" => "permission_group.users",
        "provider" => "permission_group.providers",
        "listing" | "service" => "permission_group.listings",
        "category" | "location" | "price" | "alert" => "permission_group.catalogue",
        "inquiry" | "review" | "job" => "permission_group.feedback",
        "report" | "partner" => "permission_group.business",
        "own" => "permission_group.own",
        _ => "permission_group.other",
    }
}

#[derive(Debug, Serialize)]
pub struct GrantGroup {
    #[serde(skip)]
    key: &'static str,
    pub label: String,
    pub permissions: Vec<GrantDto>,
}

/// Consecutive permissions with the same heading, in catalogue order.
fn grouped(locale: &Locale, permissions: Vec<GrantDto>) -> Vec<GrantGroup> {
    let mut groups: Vec<GrantGroup> = Vec::new();
    for p in permissions {
        let key = group_key(&p.code);
        match groups.iter_mut().find(|g| g.key == key) {
            Some(g) => g.permissions.push(p),
            None => groups.push(GrantGroup {
                key,
                label: locale.text(key),
                permissions: vec![p],
            }),
        }
    }
    groups
}

/// The keys ticked in a submitted grid.
fn ticked(form: &HashMap<String, String>) -> Vec<String> {
    form.keys()
        .filter_map(|k| k.strip_prefix(FIELD_PREFIX))
        .map(str::to_owned)
        .collect()
}

#[derive(Serialize)]
pub struct RolesPage {
    pub tabs: Vec<AdminTab>,
    pub roles: Vec<RoleRow>,
}

/// The value of `?tab=` for every permission against every role.
pub const TAB_PERMISSIONS: &str = "permissions";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RolesQuery {
    /// `permissions`: the second tab, the read-only grid of every role.
    pub tab: String,
}

/// The page's two tabs: the roles, and every permission against them.
fn tabs(state: &AppState, grid: bool) -> Result<Vec<AdminTab>, Error> {
    Ok(vec![
        AdminTab {
            label: state.locale().text("admin_roles.tab_roles"),
            href: url("admin_roles", &[])?,
            count: None,
            current: !grid,
        },
        AdminTab {
            label: state.locale().text("admin_roles.tab_permissions"),
            href: ListPosition {
                tab: TAB_PERMISSIONS.to_owned(),
                ..ListPosition::default()
            }
            .href("admin_roles", &[])?,
            count: None,
            current: grid,
        },
    ])
}

#[derive(Serialize)]
pub struct RoleRow {
    pub role: RoleDto,
    pub granted: usize,
    pub href: String,
}

/// GET /admin/roles (`?tab=permissions`: the grid of every permission)
pub async fn list(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<RolesQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    if q.tab == TAB_PERMISSIONS {
        return grid_page(&state, &actor).await;
    }
    let matrix = state
        .usecases
        .permission
        .matrix(&actor)
        .await?
        .worded(state.locale());
    let mut roles = Vec::with_capacity(matrix.roles.len());
    for (i, role) in matrix.roles.into_iter().enumerate() {
        let granted = matrix
            .rows
            .iter()
            .filter(|r| r.granted.get(i).copied().unwrap_or(false))
            .count();
        let href = url("admin_role", &[("id", &role.id.to_string())])?;
        roles.push(RoleRow {
            role,
            granted,
            href,
        });
    }
    render_admin(
        &state,
        &actor,
        &url("admin_roles", &[])?,
        StatusCode::OK,
        "pages/admin/roles.html",
        &RolesPage {
            tabs: tabs(&state, false)?,
            roles,
        },
    )
}

#[derive(Serialize)]
pub struct RolePage {
    pub role: RoleDto,
    pub groups: Vec<GrantGroup>,
    pub field_prefix: &'static str,
    /// The visitor may change the grid; otherwise it is shown read-only.
    pub editable: bool,
    pub action: String,
    pub errors: FormErrors,
    pub saved: bool,
}

async fn role_page(
    state: &AppState,
    actor: &application::authz::Actor,
    role_id: i32,
    errors: FormErrors,
    saved: bool,
    status: StatusCode,
) -> Result<Response, Error> {
    let RoleGrantsDto { role, permissions } = state
        .usecases
        .permission
        .role_grants(actor, role_id)
        .await?
        .worded(state.locale());
    let id = role_id.to_string();
    let page = RolePage {
        groups: grouped(state.locale(), permissions),
        field_prefix: FIELD_PREFIX,
        editable: actor.can(PERM_ROLE_PERMISSION_MANAGE),
        action: url("admin_role_permissions", &[("id", &id)])?,
        role,
        errors,
        saved,
    };
    render_admin(
        state,
        actor,
        &url("admin_role", &[("id", &id)])?,
        status,
        "pages/admin/role.html",
        &page,
    )
}

/// GET /admin/roles/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::RoleNotFound)?;
    let saved = flash.0 == Some(Flash::RoleSaved);
    let mut response = role_page(
        &state,
        &actor,
        id,
        FormErrors::default(),
        saved,
        StatusCode::OK,
    )
    .await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/roles/{id}/permissions
pub async fn save(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<HashMap<String, String>>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::RoleNotFound)?;
    let reason = form
        .get("reason")
        .map(|r| r.trim().to_owned())
        .filter(|r| !r.is_empty());
    match state
        .usecases
        .permission
        .set_role_grants(&actor, id, ticked(&form), reason)
        .await
    {
        Ok(()) => {
            let mut response = see_other(&url("admin_role", &[("id", &id.to_string())])?)?;
            Flash::RoleSaved.set_on(&mut response);
            Ok(response)
        }
        // Missing the manage permission is the error page; a refused change
        // (own access, unknown key) is shown on the grid.
        Err(err) if is_form_error(&err) && err.code() != ErrorCode::PermissionDenied => {
            let errors = FormErrors::from_error(&state, &err);
            role_page(&state, &actor, id, errors, false, err.kind().status()).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Serialize)]
pub struct PermissionsPage {
    pub tabs: Vec<AdminTab>,
    pub matrix: PermissionMatrixDto,
    pub role_hrefs: Vec<String>,
}

/// GET /admin/permissions → /admin/roles?tab=permissions (moved for good).
pub async fn matrix() -> Result<Response, Error> {
    moved_permanently(
        &ListPosition {
            tab: TAB_PERMISSIONS.to_owned(),
            ..ListPosition::default()
        }
        .href("admin_roles", &[])?,
    )
}

/// The roles page's second tab: every permission against every role.
async fn grid_page(state: &AppState, actor: &application::authz::Actor) -> Result<Response, Error> {
    let matrix = state
        .usecases
        .permission
        .matrix(actor)
        .await?
        .worded(state.locale());
    let role_hrefs = matrix
        .roles
        .iter()
        .map(|r| url("admin_role", &[("id", &r.id.to_string())]))
        .collect::<Result<_, _>>()?;
    render_admin(
        state,
        actor,
        &url("admin_roles", &[])?,
        StatusCode::OK,
        "pages/admin/permissions.html",
        &PermissionsPage {
            tabs: tabs(state, true)?,
            matrix,
            role_hrefs,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(code: &str) -> GrantDto {
        GrantDto {
            code: code.into(),
            name: code.into(),
            granted: false,
            grantable: true,
        }
    }

    #[test]
    fn permissions_are_grouped_by_key_prefix_in_order() -> Result<(), toml::de::Error> {
        let vi = Locale::vi()?;
        let groups = grouped(
            &vi,
            vec![
                grant("admin.dashboard.view"),
                grant("user.read"),
                grant("role.read"),
                grant("own.listing.create"),
            ],
        );
        let shape: Vec<(&str, usize)> = groups
            .iter()
            .map(|g| (g.key, g.permissions.len()))
            .collect();
        assert_eq!(
            shape,
            [
                ("permission_group.system", 1),
                ("permission_group.users", 2),
                ("permission_group.own", 1)
            ]
        );
        assert!(groups.iter().all(|g| g.label == vi.text(g.key)));
        Ok(())
    }

    #[test]
    fn only_prefixed_boxes_count_as_ticked() {
        let form: HashMap<String, String> = [
            ("perm.user.read".to_owned(), "on".to_owned()),
            ("reason".to_owned(), "x".to_owned()),
        ]
        .into();
        assert_eq!(ticked(&form), ["user.read"]);
    }
}
