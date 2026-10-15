//! The admin panel: one layout (`layouts/admin.html`) for every admin page.
//! Its menu shows only what the visitor's permissions open; the pages still
//! check access themselves through the use cases.

pub mod dashboard;
pub mod roles;

use application::{
    authz::Actor,
    catalogue::{
        PERM_ADMIN_DASHBOARD_VIEW, PERM_CATEGORY_MANAGE, PERM_INQUIRY_READ, PERM_LISTING_MODERATE,
        PERM_LOCATION_MANAGE, PERM_PROVIDER_READ, PERM_REVIEW_MODERATE, PERM_ROLE_READ,
        PERM_SERVICE_MANAGE, PERM_SYSTEM_DEMO_MANAGE, PERM_USER_READ,
    },
};
use axum::{http::StatusCode, response::Response};
use domain::Error;
use serde::Serialize;

use crate::{app_state::AppState, routes, shell::Shell, views::render_page_status};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NavItem {
    pub href: String,
    pub icon: &'static str,
    pub label: &'static str,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NavGroup {
    /// `None` for top-level entries (the dashboard).
    pub label: Option<&'static str>,
    pub items: Vec<NavItem>,
}

/// The admin frame: who is signed in and the menu they may use.
#[derive(Debug, Clone, Serialize)]
pub struct AdminNav {
    pub username: String,
    pub groups: Vec<NavGroup>,
}

/// (route, icon, label, permission that opens it).
type Entry = (&'static str, &'static str, &'static str, &'static str);

const MENU: &[(Option<&str>, &[Entry])] = &[
    (
        None,
        &[(
            "admin_dashboard",
            "fa-gauge",
            "Dashboard",
            PERM_ADMIN_DASHBOARD_VIEW,
        )],
    ),
    (
        Some("Nội dung"),
        &[
            (
                "admin_explore",
                "fa-store",
                "Nhà cung cấp",
                PERM_PROVIDER_READ,
            ),
            (
                "admin_products",
                "fa-box-open",
                "Sản phẩm",
                PERM_LISTING_MODERATE,
            ),
            (
                "admin_services",
                "fa-screwdriver-wrench",
                "Dịch vụ",
                PERM_SERVICE_MANAGE,
            ),
            (
                "admin_categories",
                "fa-tags",
                "Danh mục",
                PERM_CATEGORY_MANAGE,
            ),
            (
                "admin_inquiries",
                "fa-envelope-open-text",
                "Yêu cầu",
                PERM_INQUIRY_READ,
            ),
            ("admin_reviews", "fa-star", "Đánh giá", PERM_REVIEW_MODERATE),
            (
                "admin_locations",
                "fa-map-location-dot",
                "Khu vực",
                PERM_LOCATION_MANAGE,
            ),
        ],
    ),
    (
        Some("Hệ thống"),
        &[
            ("admin_users", "fa-users", "Người dùng", PERM_USER_READ),
            ("admin_roles", "fa-shield-halved", "Vai trò", PERM_ROLE_READ),
            ("admin_permissions", "fa-key", "Quyền hạn", PERM_ROLE_READ),
            (
                "setup_seed",
                "fa-database",
                "Dữ liệu mẫu",
                PERM_SYSTEM_DEMO_MANAGE,
            ),
        ],
    ),
];

impl AdminNav {
    pub fn new(actor: &Actor, path: &str) -> Self {
        let groups = MENU
            .iter()
            .map(|(label, entries)| NavGroup {
                label: *label,
                items: entries
                    .iter()
                    .filter(|(_, _, _, perm)| actor.can(perm))
                    .filter_map(|(route, icon, label, _)| {
                        let href = routes::url(route, &[]).ok()?;
                        Some(NavItem {
                            active: path == href || path.starts_with(&format!("{href}/")),
                            href,
                            icon,
                            label,
                        })
                    })
                    .collect(),
            })
            .filter(|g| !g.items.is_empty())
            .collect();
        Self {
            username: actor.username.clone(),
            groups,
        }
    }
}

/// What every admin template gets: the frame and the page's own data.
#[derive(Serialize)]
struct AdminPage<'a, T: Serialize> {
    admin: AdminNav,
    body: &'a T,
}

/// Renders an admin page for `actor` at `path`.
pub fn render_admin<T: Serialize>(
    state: &AppState,
    actor: &Actor,
    path: &str,
    status: StatusCode,
    template: &str,
    body: &T,
) -> Result<Response, Error> {
    let page = AdminPage {
        admin: AdminNav::new(actor, path),
        body,
    };
    render_page_status(
        state,
        &Shell::without_session(path, None),
        status,
        template,
        &page,
    )
}

pub fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("admin links: {e}")))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use domain::models::user::ActorRecord;

    use super::*;

    fn actor(permissions: &[&str]) -> Actor {
        Actor::from_record(ActorRecord {
            user_id: 1,
            username: "quantri".into(),
            role_code: "any".into(),
            is_active: true,
            permissions: permissions.iter().map(|p| (*p).to_owned()).collect(),
        })
        .expect("an active record is an actor")
    }

    #[test]
    fn the_menu_shows_only_what_the_permissions_open() {
        let nav = AdminNav::new(
            &actor(&[PERM_ADMIN_DASHBOARD_VIEW, PERM_ROLE_READ]),
            "/admin/roles/2",
        );
        let labels: Vec<&str> = nav
            .groups
            .iter()
            .flat_map(|g| g.items.iter().map(|i| i.label))
            .collect();
        assert_eq!(labels, ["Dashboard", "Vai trò", "Quyền hạn"]);
        assert_eq!(nav.groups.len(), 2, "the empty content group is hidden");
        let active: Vec<&str> = nav
            .groups
            .iter()
            .flat_map(|g| g.items.iter().filter(|i| i.active).map(|i| i.label))
            .collect();
        assert_eq!(active, ["Vai trò"], "a sub-page keeps its section active");
    }
}
