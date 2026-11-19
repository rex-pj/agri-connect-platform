//! The admin panel: one layout (`layouts/admin.html`) for every admin page.
//! Its menu shows only what the visitor's permissions open; the pages still
//! check access themselves through the use cases.

pub mod alert_kinds;
pub mod alerts;
pub mod categories;
pub mod coded;
pub mod commodities;
pub mod crops;
pub mod dashboard;
pub mod demand;
pub mod inquiries;
pub mod jobs;
pub mod locations;
pub mod partners;
pub mod prices;
pub mod products;
pub mod providers;
pub mod reviews;
pub mod roles;
pub mod sectors;
pub mod services;
pub mod settings;
pub mod users;

use application::{
    authz::Actor,
    catalogue::{
        PERM_ADMIN_DASHBOARD_VIEW, PERM_ALERT_MANAGE, PERM_CATEGORY_MANAGE, PERM_INQUIRY_READ,
        PERM_JOB_MODERATE, PERM_LISTING_MODERATE, PERM_LOCATION_MANAGE, PERM_PARTNER_MANAGE,
        PERM_PRICE_MANAGE, PERM_PROVIDER_READ, PERM_REPORT_DEMAND_READ, PERM_REVIEW_MODERATE,
        PERM_ROLE_READ, PERM_SERVICE_MANAGE, PERM_SETTING_MANAGE, PERM_SYSTEM_DEMO_MANAGE,
        PERM_USER_READ,
    },
    usecases::admin_usecase::HistoryOf,
};
use axum::{http::StatusCode, response::Response};
use domain::{
    constants::{CATEGORY_TYPE_PRODUCT, CATEGORY_TYPE_SERVICE},
    Error, ErrorCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::IMAGE_FIELD,
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
    /// What else the menu's search finds this entry by: the names of the
    /// lists behind it (its tabs). Never shown.
    pub keywords: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NavGroup {
    /// `None` for top-level entries (the dashboard).
    pub label: Option<String>,
    /// The group's stable name (its label's text key), under which the
    /// browser remembers that it was folded.
    pub key: Option<&'static str>,
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

/// Grouped by which way the thing flows, so that what belongs together sits
/// together: what farmers sent in (to answer), what the admin publishes to
/// them (prices, alerts), the listings, the B2B figures, the reference data
/// the rest is filed under, then accounts and access. The classification is
/// one entry: the sectors and what hangs on them (commodities, service types,
/// supply types) are the tabs of one screen (`catalogue_tabs`). Pages without
/// an entry light their owner's (`MENU_OWNERS`).
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
        Some("admin_menu.inbox"),
        &[
            (
                "admin_inquiries",
                "fa-envelope-open-text",
                "admin_menu.inquiries",
                PERM_INQUIRY_READ,
            ),
            (
                "admin_jobs",
                "fa-bullhorn",
                "admin_menu.jobs",
                PERM_JOB_MODERATE,
            ),
            (
                "admin_reviews",
                "fa-star",
                "admin_menu.reviews",
                PERM_REVIEW_MODERATE,
            ),
        ],
    ),
    (
        Some("admin_menu.published"),
        &[
            (
                "admin_prices",
                "fa-chart-line",
                "admin_menu.prices",
                PERM_PRICE_MANAGE,
            ),
            (
                "admin_alerts",
                "fa-triangle-exclamation",
                "admin_menu.alerts",
                PERM_ALERT_MANAGE,
            ),
        ],
    ),
    (
        Some("admin_menu.directory"),
        &[
            (
                "admin_providers",
                "fa-store",
                "admin_menu.providers",
                PERM_PROVIDER_READ,
            ),
            (
                "admin_services",
                "fa-screwdriver-wrench",
                "admin_menu.services",
                PERM_SERVICE_MANAGE,
            ),
            (
                "admin_products",
                "fa-box-open",
                "admin_menu.products",
                PERM_LISTING_MODERATE,
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
        Some("admin_menu.setup"),
        &[
            (
                "admin_sectors",
                "fa-sitemap",
                "admin_menu.catalogue",
                PERM_CATEGORY_MANAGE,
            ),
            (
                "admin_locations",
                "fa-map-location-dot",
                "admin_menu.locations",
                PERM_LOCATION_MANAGE,
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
                "setup_seed",
                "fa-database",
                "admin_menu.demo_data",
                PERM_SYSTEM_DEMO_MANAGE,
            ),
            (
                "admin_settings",
                "fa-sliders",
                "admin_menu.settings",
                PERM_SETTING_MANAGE,
            ),
        ],
    ),
];

/// The address of the menu entry `route`, when `actor` may open it: the
/// menu's own permission, so a link elsewhere (a dashboard card) never
/// leads where the menu would not.
pub fn menu_href(actor: &Actor, route: &str) -> Option<String> {
    MENU.iter()
        .flat_map(|(_, entries)| entries.iter())
        .find(|(r, _, _, perm)| *r == route && actor.can(perm))
        .and_then(|(r, _, _, _)| routes::url(r, &[]).ok())
}

/// Pages with no menu entry of their own, and the entry that lights for them:
/// a crop is kept on the price board's second tab, and the classification's
/// entry leads to its first tab, the sectors.
const MENU_OWNERS: &[(&str, &str)] = &[
    ("admin_crops", "admin_prices"),
    ("admin_alert_kinds", "admin_alerts"),
    ("admin_commodities", "admin_sectors"),
    ("admin_categories", "admin_sectors"),
];

/// What the menu's search also finds an entry by: the lists kept on its tabs,
/// which have no entry of their own (menu route, text keys).
const MENU_KEYWORDS: &[(&str, &[&str])] = &[
    (
        "admin_prices",
        &["admin_menu.price_board", "admin_menu.crops"],
    ),
    ("admin_alerts", &["alert.tab_kinds"]),
    (
        "admin_sectors",
        &[
            "admin_menu.sectors",
            "admin_menu.commodities",
            "admin_menu.service_types",
            "admin_menu.supply_types",
        ],
    ),
    ("admin_roles", &["admin_menu.permissions"]),
    (
        "admin_settings",
        &[
            "settings.group.support",
            "settings.group.access",
            "settings.group.jobs",
            "settings.group.listings",
            "settings.group.market",
        ],
    ),
];

/// The search words of the menu entry leading to `route`.
fn keywords(locale: &Locale, route: &str) -> String {
    MENU_KEYWORDS
        .iter()
        .filter(|(entry, _)| *entry == route)
        .flat_map(|(_, keys)| keys.iter().map(|key| locale.text(key)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A tab of the classification screen: the sectors, then what is filed under
/// a sector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogueTab {
    Sectors,
    Commodities,
    ServiceTypes,
    SupplyTypes,
}

/// The classification's tabs, the same on its four lists, `current` marked:
/// each list keeps its own address.
pub fn catalogue_tabs(state: &AppState, current: CatalogueTab) -> Result<Vec<AdminTab>, Error> {
    let categories = url("admin_categories", &[])?;
    let of_type = |kind: &str| with_query(&categories, &[("type", kind)]);
    let tabs = [
        (
            CatalogueTab::Sectors,
            "admin_menu.sectors",
            url("admin_sectors", &[])?,
        ),
        (
            CatalogueTab::Commodities,
            "admin_menu.commodities",
            url("admin_commodities", &[])?,
        ),
        (
            CatalogueTab::ServiceTypes,
            "admin_menu.service_types",
            of_type(CATEGORY_TYPE_SERVICE),
        ),
        (
            CatalogueTab::SupplyTypes,
            "admin_menu.supply_types",
            of_type(CATEGORY_TYPE_PRODUCT),
        ),
    ];
    Ok(tabs
        .into_iter()
        .map(|(tab, label, href)| AdminTab {
            label: state.locale().text(label),
            href,
            count: None,
            current: tab == current,
        })
        .collect())
}

/// The admin page of the sector `code` names; none for every sector.
pub fn sector_href(
    sectors: &[application::outputs::CodedDto],
    code: Option<&str>,
) -> Result<Option<String>, Error> {
    code.and_then(|c| sectors.iter().find(|s| s.code == c))
        .map(|s| url("admin_sector", &[("id", &s.id.to_string())]))
        .transpose()
}

/// `path` itself, or the address of the menu entry that owns it.
fn menu_path(path: &str) -> String {
    MENU_OWNERS
        .iter()
        .find_map(|(page, owner)| {
            let href = routes::url(page, &[]).ok()?;
            (path == href || path.starts_with(&format!("{href}/")))
                .then(|| routes::url(owner, &[]).ok())
                .flatten()
        })
        .unwrap_or_else(|| path.to_owned())
}

impl AdminNav {
    pub fn new(locale: &Locale, actor: &Actor, path: &str) -> Self {
        let lit = menu_path(path);
        let path = lit.as_str();
        let groups = MENU
            .iter()
            .map(|(label, entries)| NavGroup {
                label: label.map(|key| locale.text(key)),
                key: *label,
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
                            keywords: keywords(locale, route),
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

/// The admin page of an account linked to a record (a provider's managing
/// account, a partner's report account), when the visitor may read accounts.
pub fn user_href(actor: &Actor, user_id: Option<i32>) -> Result<Option<String>, Error> {
    match user_id {
        Some(id) if actor.can(PERM_USER_READ) => {
            Some(url("admin_user", &[("id", &id.to_string())])).transpose()
        }
        _ => Ok(None),
    }
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

    /// A refused photo (wrong type, too large): the boxes as typed, the
    /// message on the photo box (a file box never keeps its file).
    pub fn photo_refused(state: &AppState, err: &Error, form: F) -> Self {
        Self {
            form: Some(form),
            errors: FormErrors::on_field(state, err, IMAGE_FIELD),
            confirm_open: false,
            flash: None,
            status: err.kind().status(),
        }
    }
}

/// One change in a record's history on its admin page: `action` is the
/// audit code (templates pick the latest of a kind by it), `label` its words
/// (`audit.<action>`, the code itself for one with no words yet).
#[derive(Serialize)]
pub struct HistoryRow {
    pub action: String,
    pub label: String,
    pub reason: Option<String>,
    pub actor: Option<String>,
    pub created_on: chrono::DateTime<chrono::Utc>,
}

/// The latest changes to `of`, worded in the request's language.
pub async fn history_rows(
    state: &AppState,
    actor: &Actor,
    of: HistoryOf,
) -> Result<Vec<HistoryRow>, Error> {
    let locale = state.locale();
    Ok(state
        .usecases
        .admin
        .history(actor, of)
        .await?
        .into_iter()
        .map(|e| HistoryRow {
            label: locale.label(&format!("audit.{}", e.action), &e.action),
            action: e.action,
            reason: e.reason,
            actor: e.actor,
            created_on: e.created_on,
        })
        .collect())
}

/// A form that only carries the optional reason of an audited change (a
/// confirmation with nothing else to send).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReasonForm {
    pub reason: String,
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

/// A tab of an admin page that shows one of several lists
/// (`molecules/admin_tabs.html`): a link, its word, maybe a count, and
/// whether it is the one shown.
#[derive(Debug, Serialize)]
pub struct AdminTab {
    pub label: String,
    pub href: String,
    pub count: Option<usize>,
    pub current: bool,
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
    /// A list narrowed to one crop (the price list).
    pub crop_id: String,
    /// A list narrowed to one commodity, or one service or supply type.
    pub commodity_id: String,
    pub category_id: String,
    /// Which list of a page with tabs (`?tab=`).
    pub tab: String,
    /// The name search box (`q`), kept like the page and the provider.
    pub q: String,
    /// A list narrowed to one status (`?state=`: contact requests, job
    /// posts); not `status`, the field a row's form sets.
    pub state: String,
}

impl ListPosition {
    /// The query that reproduces this position (nothing for page 1 and no
    /// filter).
    pub fn query(&self) -> Vec<(&'static str, String)> {
        let mut q = Vec::new();
        if !self.tab.trim().is_empty() {
            q.push(("tab", self.tab.trim().to_owned()));
        }
        if let Some(c) = parse_opt::<i32>(&self.crop_id).filter(|c| *c > 0) {
            q.push(("crop_id", c.to_string()));
        }
        if let Some(c) = parse_opt::<i32>(&self.commodity_id).filter(|c| *c > 0) {
            q.push(("commodity_id", c.to_string()));
        }
        if let Some(c) = parse_opt::<i32>(&self.category_id).filter(|c| *c > 0) {
            q.push(("category_id", c.to_string()));
        }
        if let Some(p) = parse_opt::<i32>(&self.provider_id).filter(|p| *p > 0) {
            q.push(("provider_id", p.to_string()));
        }
        if let Some(s) = self.search() {
            q.push(("q", s.to_owned()));
        }
        if let Some(s) = self.state() {
            q.push(("state", s.to_owned()));
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
                    icon: "fa-store",
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

    /// The status filter as typed, when it is a code (lower-case letters
    /// and `_`); the page checks it against its list (`state_in`).
    fn state(&self) -> Option<&str> {
        Some(self.state.trim())
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
    }

    /// The status filter when it is one of `statuses`, else none (the
    /// whole list).
    pub fn state_in(&self, statuses: &[&'static str]) -> Option<&'static str> {
        self.state()
            .and_then(|s| statuses.iter().copied().find(|code| *code == s))
    }

    /// The chips above a list filtered by status (`molecules/status_chips`):
    /// the whole list (`all`), then each status worded by `word`; each
    /// keeps the provider filter and goes back to the first page. `waiting`:
    /// the status still waiting for someone and how many are in it, counted
    /// on its chip.
    pub fn state_chips(
        &self,
        route: &str,
        statuses: &[&'static str],
        all: String,
        word: impl Fn(&str) -> String,
        waiting: (&str, i64),
    ) -> Result<Vec<AdminTab>, Error> {
        let current = self.state_in(statuses);
        let chip = |state: Option<&str>, label: String| {
            let at = ListPosition {
                page: String::new(),
                state: state.unwrap_or_default().to_owned(),
                ..self.clone()
            };
            Ok(AdminTab {
                label,
                href: at.href(route, &[])?,
                count: (state == Some(waiting.0)).then(|| usize::try_from(waiting.1).unwrap_or(0)),
                current: current == state,
            })
        };
        std::iter::once(chip(None, all))
            .chain(statuses.iter().map(|s| chip(Some(s), word(s))))
            .collect()
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
    /// Font Awesome class of what the list is narrowed to.
    pub icon: &'static str,
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
            ..ListPosition::default()
        };
        assert_eq!(
            p.href("admin_products", &[("hide", "9")]).ok().as_deref(),
            Some("/admin/supplies?provider_id=7&hide=9")
        );
        let p = ListPosition {
            page: "2".into(),
            provider_id: "7".into(),
            q: " phan bon ".into(),
            ..ListPosition::default()
        };
        assert_eq!(
            p.href("admin_products", &[]).ok().as_deref(),
            Some("/admin/supplies?provider_id=7&q=phan+bon&page=2"),
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
            [vi.text("admin_menu.dashboard"), roles.clone()],
            "roles and their permissions are one entry"
        );
        assert_eq!(nav.groups.len(), 2, "the empty groups are hidden");
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

        // A crop has no entry of its own: the price board's lights.
        let nav = AdminNav::new(
            &vi,
            &actor(&[PERM_ADMIN_DASHBOARD_VIEW, PERM_PRICE_MANAGE]),
            "/admin/crops/3",
        );
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
        assert_eq!(active, [vi.text("admin_menu.prices")]);
        let keys: Vec<Option<&str>> = nav.groups.iter().map(|g| g.key).collect();
        assert_eq!(
            keys,
            [None, Some("admin_menu.published")],
            "the dashboard is in no group; a group folds under its key"
        );
        let found_by_tab_lists = nav
            .groups
            .iter()
            .flat_map(|g| &g.items)
            .any(|i| i.keywords.contains(&vi.text("admin_menu.crops")));
        assert!(
            found_by_tab_lists,
            "the search finds an entry by the lists on its tabs"
        );

        // The classification is one entry, lit on each of its four lists.
        for path in [
            "/admin/sectors",
            "/admin/commodities/4",
            "/admin/categories",
        ] {
            let nav = AdminNav::new(&vi, &actor(&[PERM_CATEGORY_MANAGE]), path);
            let entries: Vec<(&str, bool)> = nav
                .groups
                .iter()
                .flat_map(|g| g.items.iter().map(|i| (i.label.as_str(), i.active)))
                .collect();
            assert_eq!(
                entries,
                [(vi.text("admin_menu.catalogue").as_str(), true)],
                "{path}"
            );
        }
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
