//! The admin panel: one layout (`layouts/admin.html`) for every admin page.
//! Its menu shows only what the visitor's permissions open; the pages still
//! check access themselves through the use cases.

pub mod categories;
pub mod crops;
pub mod dashboard;
pub mod demand;
pub mod inquiries;
pub mod locations;
pub mod partners;
pub mod prices;
pub mod products;
pub mod providers;
pub mod reviews;
pub mod roles;
pub mod services;
pub mod users;

use application::{
    authz::Actor,
    catalogue::{
        PERM_ADMIN_DASHBOARD_VIEW, PERM_CATEGORY_MANAGE, PERM_INQUIRY_READ, PERM_LISTING_MODERATE,
        PERM_LOCATION_MANAGE, PERM_PARTNER_MANAGE, PERM_PRICE_MANAGE, PERM_PROVIDER_READ,
        PERM_REPORT_DEMAND_READ, PERM_REVIEW_MODERATE, PERM_ROLE_READ, PERM_SERVICE_MANAGE,
        PERM_SYSTEM_DEMO_MANAGE, PERM_USER_READ,
    },
};
use axum::{http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::Flash,
    locale::Locale,
    pages::{
        forms::{is_form_error, FormErrors},
        listing::{parse_opt, with_query, Pagination},
        see_other,
    },
    routes,
    shell::{Link, ProfileMenu, Shell},
    views::render_page_status,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NavItem {
    pub href: String,
    pub icon: &'static str,
    pub label: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NavGroup {
    /// `None` for top-level entries (the dashboard).
    pub label: Option<String>,
    pub items: Vec<NavItem>,
}

/// The admin frame: who is signed in and the menu they may use.
#[derive(Debug, Clone, Serialize)]
pub struct AdminNav {
    pub username: String,
    /// The bar's account menu (the same molecule as the app's): the profile,
    /// then logout back to the login page.
    pub account: ProfileMenu,
    pub groups: Vec<NavGroup>,
}

/// (route, icon, label text key) of the account menu's links. The public
/// site is not one: it is its own button in the bar.
const ACCOUNT_LINKS: &[(&str, &str, &str)] = &[("account", "fa-id-card", "account_menu.profile")];

/// (route, icon, label text key, permission that opens it).
type Entry = (&'static str, &'static str, &'static str, &'static str);

const MENU: &[(Option<&str>, &[Entry])] = &[
    (
        None,
        &[(
            "admin_dashboard",
            "fa-gauge",
            "admin_menu.dashboard",
            PERM_ADMIN_DASHBOARD_VIEW,
        )],
    ),
    (
        Some("admin_menu.content"),
        &[
            (
                "admin_explore",
                "fa-store",
                "admin_menu.providers",
                PERM_PROVIDER_READ,
            ),
            (
                "admin_products",
                "fa-box-open",
                "admin_menu.products",
                PERM_LISTING_MODERATE,
            ),
            (
                "admin_services",
                "fa-screwdriver-wrench",
                "admin_menu.services",
                PERM_SERVICE_MANAGE,
            ),
            (
                "admin_categories",
                "fa-tags",
                "admin_menu.categories",
                PERM_CATEGORY_MANAGE,
            ),
            (
                "admin_inquiries",
                "fa-envelope-open-text",
                "admin_menu.inquiries",
                PERM_INQUIRY_READ,
            ),
            (
                "admin_reviews",
                "fa-star",
                "admin_menu.reviews",
                PERM_REVIEW_MODERATE,
            ),
            (
                "admin_locations",
                "fa-map-location-dot",
                "admin_menu.locations",
                PERM_LOCATION_MANAGE,
            ),
            (
                "admin_prices",
                "fa-chart-line",
                "admin_menu.prices",
                PERM_PRICE_MANAGE,
            ),
        ],
    ),
    (
        Some("admin_menu.business"),
        &[
            (
                "admin_demand",
                "fa-chart-column",
                "admin_menu.demand",
                PERM_REPORT_DEMAND_READ,
            ),
            (
                "admin_partners",
                "fa-handshake",
                "admin_menu.partners",
                PERM_PARTNER_MANAGE,
            ),
        ],
    ),
    (
        Some("admin_menu.system"),
        &[
            (
                "admin_users",
                "fa-users",
                "admin_menu.users",
                PERM_USER_READ,
            ),
            (
                "admin_roles",
                "fa-shield-halved",
                "admin_menu.roles",
                PERM_ROLE_READ,
            ),
            (
                "admin_permissions",
                "fa-key",
                "admin_menu.permissions",
                PERM_ROLE_READ,
            ),
            (
                "setup_seed",
                "fa-database",
                "admin_menu.demo_data",
                PERM_SYSTEM_DEMO_MANAGE,
            ),
        ],
    ),
];

impl AdminNav {
    pub fn new(locale: &Locale, actor: &Actor, path: &str) -> Self {
        let groups = MENU
            .iter()
            .map(|(label, entries)| NavGroup {
                label: label.map(|key| locale.text(key)),
                items: entries
                    .iter()
                    .filter(|(_, _, _, perm)| actor.can(perm))
                    .filter_map(|(route, icon, label, _)| {
                        let href = routes::url(route, &[]).ok()?;
                        Some(NavItem {
                            active: path == href || path.starts_with(&format!("{href}/")),
                            href,
                            icon,
                            label: locale.text(label),
                        })
                    })
                    .collect(),
            })
            .filter(|g| !g.items.is_empty())
            .collect();
        let account = ProfileMenu {
            signed_in: true,
            items: ACCOUNT_LINKS
                .iter()
                .filter_map(|(route, icon, label)| {
                    Some(Link {
                        href: routes::url(route, &[]).ok()?,
                        icon,
                        label: locale.text(label),
                    })
                })
                .collect(),
            logout: routes::url("admin_logout", &[]).ok(),
        };
        Self {
            username: actor.username.clone(),
            account,
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
        admin: AdminNav::new(state.locale(), actor, path),
        body,
    };
    render_page_status(
        state,
        &Shell::without_session(state.locale(), path, None),
        status,
        template,
        &page,
    )
}

pub fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("admin links: {e}")))
}

/// A refusal the admin can act on from the page (a broken rule, a conflict):
/// the page is re-shown with it. A missing permission is not one: it gets
/// the 403 page.
pub fn is_refusal(err: &Error) -> bool {
    is_form_error(err) && err.code() != ErrorCode::PermissionDenied
}

/// How an edit page comes back: fresh (with a flash), or after a refused
/// change (with the values as typed and the messages).
pub struct Outcome<F> {
    /// The values as typed; `None` shows the stored ones.
    pub form: Option<F>,
    pub errors: FormErrors,
    /// The confirmation dialog re-opens after a refused confirmation.
    pub confirm_open: bool,
    pub flash: Option<Flash>,
    pub status: StatusCode,
}

impl<F> Default for Outcome<F> {
    fn default() -> Self {
        Self {
            form: None,
            errors: FormErrors::default(),
            confirm_open: false,
            flash: None,
            status: StatusCode::OK,
        }
    }
}

impl<F> Outcome<F> {
    pub fn refused(state: &AppState, err: &Error, form: Option<F>, confirm_open: bool) -> Self {
        Self {
            form,
            errors: FormErrors::from_error(state, err),
            confirm_open,
            flash: None,
            status: err.kind().status(),
        }
    }
}

/// The value of the `back` field a row action on a list sends, for a record
/// that also has its own page: land back on the list, not on that page.
pub const BACK_TO_LIST: &str = "list";

/// The hidden fields of a row action on a list whose records have their own
/// page: `back` plus the list position (`keep` in the templates).
#[derive(Debug, Serialize)]
pub struct ListReturn {
    pub back: &'static str,
    #[serde(flatten)]
    pub position: ListPosition,
}

impl ListReturn {
    pub fn new(position: ListPosition) -> Self {
        Self {
            back: BACK_TO_LIST,
            position,
        }
    }
}

/// The row confirmation a list's query asks for (`?hide={id}` /
/// `?delete={id}`); anything not a number asks for none.
#[derive(Debug, Default, Clone, Copy)]
pub struct Asked {
    pub hide: Option<i32>,
    pub delete: Option<i32>,
}

impl Asked {
    pub fn new(hide: &str, delete: &str) -> Self {
        Self {
            hide: parse_opt(hide),
            delete: parse_opt(delete),
        }
    }
}

/// The row confirmation open on this render of a list (`?hide=` or
/// `?delete=`): which kind, the record's name for the title, and where its
/// form posts.
#[derive(Debug, Serialize)]
pub struct RowConfirm {
    /// "hide" or "delete".
    pub kind: &'static str,
    pub name: String,
    pub action: String,
}

/// Which page of an admin list a row action came from, so the redirect lands
/// back there. Sent as hidden fields and rebuilt from the numbers in them,
/// never from a posted URL, so a form cannot redirect anywhere else.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ListPosition {
    pub page: String,
    pub provider_id: String,
    /// The name search box (`q`), kept like the page and the provider.
    pub q: String,
}

impl ListPosition {
    /// The query that reproduces this position (nothing for page 1 and no
    /// filter).
    pub fn query(&self) -> Vec<(&'static str, String)> {
        let mut q = Vec::new();
        if let Some(p) = parse_opt::<i32>(&self.provider_id).filter(|p| *p > 0) {
            q.push(("provider_id", p.to_string()));
        }
        if let Some(s) = self.search() {
            q.push(("q", s.to_owned()));
        }
        if let Some(n) = parse_opt::<i64>(&self.page).filter(|n| *n > 1) {
            q.push(("page", n.to_string()));
        }
        q
    }

    /// The list at `route` with this position and `extra` parameters.
    pub fn href(&self, route: &str, extra: &[(&str, &str)]) -> Result<String, Error> {
        let query = self.query();
        let mut params: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        params.extend_from_slice(extra);
        Ok(with_query(&url(route, &[])?, &params))
    }

    /// 303 back to the list at `route`, with `flash`.
    pub fn back_to(&self, route: &str, flash: Flash) -> Result<Response, Error> {
        let mut response = see_other(&self.href(route, &[])?)?;
        flash.set_on(&mut response);
        Ok(response)
    }

    pub fn provider_id(&self) -> Option<i32> {
        parse_opt::<i32>(&self.provider_id).filter(|p| *p > 0)
    }

    /// The text typed in the search box, trimmed; `None` when blank.
    pub fn search(&self) -> Option<&str> {
        Some(self.q.trim()).filter(|q| !q.is_empty())
    }

    /// The page links of the list at `path`, keeping its provider filter and
    /// search.
    pub fn pagination(&self, path: &str, current: i64, total: i64) -> Option<Pagination> {
        let mut kept = self.query();
        kept.retain(|(k, _)| *k != "page");
        let params: Vec<(&str, &str)> = kept.iter().map(|(k, v)| (*k, v.as_str())).collect();
        Pagination::new(path, &params, current, total)
    }

    /// The search box above the list at `route` (`molecules/admin_search`):
    /// it keeps the provider filter, and its way out clears only the search.
    pub fn search_box(&self, route: &str) -> Result<SearchBox, Error> {
        let provider_id = self
            .provider_id()
            .map(|p| p.to_string())
            .unwrap_or_default();
        let without_search = ListPosition {
            provider_id: provider_id.clone(),
            ..ListPosition::default()
        };
        Ok(SearchBox {
            action: url(route, &[])?,
            q: self.search().unwrap_or_default().to_owned(),
            provider_id,
            clear_href: without_search.href(route, &[])?,
        })
    }

    /// The bar above the list at `route` when it is narrowed to one provider
    /// (`molecules/admin_filter`); `name` comes from the rows, so it is
    /// `None` when there are none.
    pub fn provider_filter(
        &self,
        route: &str,
        name: Option<String>,
    ) -> Result<Option<ProviderFilter>, Error> {
        self.provider_id()
            .map(|id| {
                Ok(ProviderFilter {
                    name,
                    href: url("admin_provider", &[("id", &id.to_string())])?,
                    clear_href: url(route, &[])?,
                })
            })
            .transpose()
    }

    pub fn page(&self) -> i64 {
        parse_opt(&self.page).unwrap_or(1)
    }
}

/// The name search above an admin list: where it submits, what was typed,
/// the provider filter it keeps (blank: none) and the list without the
/// search.
#[derive(Debug, Serialize)]
pub struct SearchBox {
    pub action: String,
    pub q: String,
    pub provider_id: String,
    pub clear_href: String,
}

/// The one provider an admin list is narrowed to: its name, its admin page,
/// and the whole list.
#[derive(Debug, Serialize)]
pub struct ProviderFilter {
    pub name: Option<String>,
    pub href: String,
    pub clear_href: String,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use domain::models::user::ActorRecord;

    use super::*;

    #[test]
    fn a_list_position_keeps_only_numbers() {
        let p = ListPosition {
            page: "3".into(),
            provider_id: "//evil.example".into(),
            ..ListPosition::default()
        };
        assert_eq!(p.query(), vec![("page", "3".to_owned())]);
        let p = ListPosition {
            page: "1".into(),
            provider_id: " 7 ".into(),
            q: "  ".into(),
        };
        assert_eq!(
            p.href("admin_products", &[("hide", "9")]).ok().as_deref(),
            Some("/admin/products?provider_id=7&hide=9")
        );
        let p = ListPosition {
            page: "2".into(),
            provider_id: "7".into(),
            q: " phan bon ".into(),
        };
        assert_eq!(
            p.href("admin_products", &[]).ok().as_deref(),
            Some("/admin/products?provider_id=7&q=phan+bon&page=2"),
            "the search is kept, trimmed and encoded"
        );
    }

    fn actor(permissions: &[&str]) -> Actor {
        Actor::from_record(ActorRecord {
            user_id: 1,
            username: "quantri".into(),
            role_code: "any".into(),
            is_active: true,
            permissions: permissions.iter().map(|p| (*p).to_owned()).collect(),
            sessions_valid_after: None,
        })
        .expect("an active record is an actor")
    }

    #[test]
    fn the_menu_shows_only_what_the_permissions_open() {
        let vi = Locale::vi().expect("vi.toml parses");
        let nav = AdminNav::new(
            &vi,
            &actor(&[PERM_ADMIN_DASHBOARD_VIEW, PERM_ROLE_READ]),
            "/admin/roles/2",
        );
        let labels: Vec<&str> = nav
            .groups
            .iter()
            .flat_map(|g| g.items.iter().map(|i| i.label.as_str()))
            .collect();
        let roles = vi.text("admin_menu.roles");
        assert_eq!(
            labels,
            [
                vi.text("admin_menu.dashboard"),
                roles.clone(),
                vi.text("admin_menu.permissions")
            ]
        );
        assert_eq!(nav.groups.len(), 2, "the empty content group is hidden");
        let active: Vec<&str> = nav
            .groups
            .iter()
            .flat_map(|g| {
                g.items
                    .iter()
                    .filter(|i| i.active)
                    .map(|i| i.label.as_str())
            })
            .collect();
        assert_eq!(active, [roles], "a sub-page keeps its section active");
        assert_eq!(
            nav.account.logout.as_deref(),
            Some("/admin/logout"),
            "the account menu signs out to the login page"
        );
        assert_eq!(
            nav.account.items.len(),
            1,
            "the profile; the public site is its own bar button"
        );
    }
}
