//! The admin panel: one layout (`layouts/admin.html`) for every admin page.
//! Its menu shows only what the visitor's permissions open; the pages still
//! check access themselves through the use cases.

pub mod alert_kinds;
pub mod alerts;
pub mod categories;
pub mod coded;
pub mod commodities;
pub mod contracts;
pub mod crops;
pub mod dashboard;
pub mod demand;
pub mod inquiries;
pub mod jobs;
pub mod listing_record;
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
    constants::SEARCH_QUERY_MAX,
    usecases::admin_usecase::HistoryOf,
};
use axum::{http::StatusCode, response::Response};
use domain::{
    constants::{CATEGORY_TYPE_PRODUCT, CATEGORY_TYPE_SERVICE},
    models::admin_list::{AdminFilter, AdminSort},
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

/// The top of one record's admin page (`organisms/record_header.html`): the
/// one place to see what state it is in and to act on it. Its states show
/// once, as badges; the actions done often are buttons beside the title, the
/// rare or harmful ones wait in the "more" menu, the harmful last.
#[derive(Serialize)]
pub struct RecordHeader {
    pub title: String,
    pub icon: String,
    pub sub: Option<String>,
    pub back_href: String,
    pub back_label: String,
    pub badges: Vec<Badge>,
    pub actions: Vec<RecordAction>,
    pub menu: Vec<RecordAction>,
}

/// One state of a record as `atoms/status_badge.html` draws it: `kind` is
/// one of its macros (`visibility`, `account`, `verified`, `managed`,
/// `demo`).
#[derive(Serialize)]
pub struct Badge {
    pub kind: &'static str,
    pub on: bool,
}

impl Badge {
    pub fn new(kind: &'static str, on: bool) -> Self {
        Self { kind, on }
    }
}

/// One action of a record header. `kind`: `link` goes to `href`; `post` is
/// a one-button form to `href` setting `field` to `value` (turning
/// something back on: no second look needed); `sheet` opens the sheet whose
/// id `href` names (`#…`), a confirmation or a form on the page. `tone` is
/// the Bootstrap colour of a button and of a menu item's icon (`success`,
/// `warning`, `danger`, `secondary`).
#[derive(Serialize)]
pub struct RecordAction {
    pub kind: &'static str,
    pub label: String,
    pub icon: &'static str,
    pub href: String,
    pub field: &'static str,
    pub value: &'static str,
    pub tone: &'static str,
}

impl RecordAction {
    pub fn link(label: String, icon: &'static str, href: String) -> Self {
        Self {
            kind: "link",
            label,
            icon,
            href,
            field: "",
            value: "",
            tone: "secondary",
        }
    }

    pub fn post(
        label: String,
        icon: &'static str,
        action: String,
        field: &'static str,
        value: &'static str,
    ) -> Self {
        Self {
            kind: "post",
            label,
            icon,
            href: action,
            field,
            value,
            tone: "success",
        }
    }

    /// The same action in another colour (a link that adds something: green).
    pub fn tinted(self, tone: &'static str) -> Self {
        Self { tone, ..self }
    }

    pub fn sheet(label: String, icon: &'static str, sheet: &str, tone: &'static str) -> Self {
        Self {
            kind: "sheet",
            label,
            icon,
            href: format!("#{sheet}"),
            field: "",
            value: "",
            tone,
        }
    }
}

/// A confirmation of a record page (`organisms/confirm_sheet.html`), opened
/// by its header: what it does in words (`effect` + `what`, else `note`),
/// the form it sends, the hidden field it sets.
#[derive(Serialize)]
pub struct Confirm {
    pub id: &'static str,
    pub title: String,
    pub icon: &'static str,
    pub action: String,
    pub button: String,
    pub button_class: &'static str,
    pub effect: &'static str,
    pub what: String,
    pub note: String,
    pub hidden_name: &'static str,
    pub hidden_value: &'static str,
    /// Open when the page comes back after this confirmation was refused.
    pub open: bool,
}

impl Confirm {
    /// Hiding or switching off: the amber button, `field` set to `false`.
    pub fn off(
        id: &'static str,
        title: String,
        icon: &'static str,
        action: String,
        button: String,
        field: &'static str,
    ) -> Self {
        Self {
            id,
            title,
            icon,
            action,
            button,
            button_class: "btn-warning",
            effect: "",
            what: String::new(),
            note: String::new(),
            hidden_name: field,
            hidden_value: "false",
            open: false,
        }
    }

    /// Deleting or removing for good: the red button.
    pub fn delete(
        id: &'static str,
        title: String,
        icon: &'static str,
        action: String,
        button: String,
    ) -> Self {
        Self {
            button_class: "btn-danger",
            hidden_name: "",
            hidden_value: "",
            ..Self::off(id, title, icon, action, button, "")
        }
    }

    pub fn saying(self, effect: &'static str, what: String) -> Self {
        Self {
            effect,
            what,
            ..self
        }
    }

    pub fn noting(self, note: String) -> Self {
        Self { note, ..self }
    }

    pub fn opened(self, open: bool) -> Self {
        Self { open, ..self }
    }
}

/// What a switch-off confirmation says: the generic "hidden from farmers,
/// can be shown again" about `what` (`confirm.hide_effect`), or its own
/// words.
pub enum Says {
    Hide(String),
    Note(String),
}

/// The on / off pair of a record (shown / hidden, active / off): while on,
/// a button opening the confirmation to turn it off; while off, a one-button
/// form turning it back on (no second look needed). The page puts the
/// button among its header's actions or in its menu.
pub struct Toggle {
    pub is_on: bool,
    /// The form both send, setting `field`.
    pub action: String,
    pub field: &'static str,
    pub sheet: &'static str,
    pub off_label: String,
    pub off_icon: &'static str,
    pub on_label: String,
    pub on_icon: &'static str,
    /// The confirmation's title and button.
    pub title: String,
    pub button: String,
    pub says: Says,
}

impl Toggle {
    /// Shown / hidden (`is_active`): "hide …" behind its confirmation, "show
    /// again" one button.
    pub fn visibility(
        state: &AppState,
        is_on: bool,
        action: String,
        sheet: &'static str,
        hide_label: String,
        title: String,
        says: Says,
    ) -> Self {
        let locale = state.locale();
        Self {
            is_on,
            action,
            field: "is_active",
            sheet,
            off_label: hide_label,
            off_icon: "fa-eye-slash",
            on_label: locale.text("admin.show_again"),
            on_icon: "fa-eye",
            title,
            button: locale.text("admin.confirm_hide"),
            says,
        }
    }

    /// The header's action, and the confirmation it opens while on.
    pub fn split(self) -> (RecordAction, Option<Confirm>) {
        if !self.is_on {
            let on =
                RecordAction::post(self.on_label, self.on_icon, self.action, self.field, "true");
            return (on, None);
        }
        let off = RecordAction::sheet(self.off_label, self.off_icon, self.sheet, "warning");
        let confirm = Confirm::off(
            self.sheet,
            self.title,
            self.off_icon,
            self.action,
            self.button,
            self.field,
        );
        let confirm = match self.says {
            Says::Hide(what) => confirm.saying("hide", what),
            Says::Note(note) => confirm.noting(note),
        };
        (off, Some(confirm))
    }
}

/// One part of a record page shown alone (`?tab=`): its code and its label's
/// locale key. The first of a page's parts is its overview, the page itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabSpec {
    pub code: &'static str,
    pub label: &'static str,
}

/// `?tab=` of a record page.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct TabQuery {
    pub tab: String,
}

/// The part asked for among `tabs`; an unknown one is the overview.
pub fn chosen_tab(tabs: &[TabSpec], asked: &str) -> &'static str {
    tabs.iter()
        .find(|t| t.code == asked.trim())
        .or(tabs.first())
        .map_or("", |t| t.code)
}

/// A part's address: the overview is the page itself.
pub fn tab_href(page: &str, tabs: &[TabSpec], code: &str) -> String {
    if tabs.first().is_some_and(|t| t.code == code) {
        page.to_owned()
    } else {
        with_query(page, &[("tab", code)])
    }
}

/// The tab links of a record page (`molecules/admin_tabs.html`), the
/// current one marked, each with the count `count` gives it.
pub fn record_tabs(
    state: &AppState,
    page: &str,
    tabs: &[TabSpec],
    current: &str,
    count: impl Fn(&str) -> Option<usize>,
) -> Vec<AdminTab> {
    tabs.iter()
        .map(|t| AdminTab {
            label: state.locale().text(t.label),
            href: tab_href(page, tabs, t.code),
            count: count(t.code),
            current: t.code == current,
        })
        .collect()
}

/// 303 to one part of a record page, with `flash`.
pub fn to_tab(page: &str, tabs: &[TabSpec], code: &str, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&tab_href(page, tabs, code))?;
    flash.set_on(&mut response);
    Ok(response)
}

/// A form that only carries the optional reason of an audited change (a
/// confirmation with nothing else to send).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReasonForm {
    pub reason: String,
}

/// The orders a queue (contact requests, reviews, job posts) offers
/// besides its own (the newest first): the oldest first, to work it
/// through in the order it came.
/// The orders a catalogue list (types, produce, areas, alerts) offers
/// besides its own: by name, the newest made first, the oldest.
pub const CATALOGUE_SORTS: &[AdminSort] = &[AdminSort::Name, AdminSort::Newest, AdminSort::Oldest];

pub const QUEUE_SORTS: &[AdminSort] = &[AdminSort::Oldest];

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
    /// A list narrowed to one area (the price list).
    pub location_id: String,
    /// Which list of a page with tabs (`?tab=`).
    pub tab: String,
    /// The name search box (`q`), kept like the page and the provider.
    pub q: String,
    /// A list narrowed to one status (`?state=`: contact requests, job
    /// posts, shown / hidden, on / off); not `status`, the field a row's
    /// form sets.
    pub state: String,
    /// The order asked for (`?sort=`, an `AdminSort` code); blank: the
    /// list's own.
    pub sort: String,
    /// Made on or after / on or before this day (`?from=`, `?to=`, as
    /// `YYYY-MM-DD`).
    pub from: String,
    pub to: String,
    /// Narrowed to one star rating, 1 to 5 (the reviews' list).
    pub rating: String,
    /// "none": only what has no reply yet (the reviews' list).
    pub reply: String,
    /// What the record is about, by code (`service` / `product`: the
    /// contact requests' list).
    pub kind: String,
    /// What a provider lacks, by code (`no_services`, `no_products`,
    /// `no_account`: the providers' list).
    pub gap: String,
    /// Narrowed to one role (the accounts' list).
    pub role_id: String,
    /// Words of the address (the providers' list): a province, a ward.
    pub area: String,
    /// Narrowed to one sector, by code (the catalogue lists).
    pub sector: String,
    /// Partners by their contracts, by code.
    pub contract: String,
    /// A monthly fee between these, in đồng (a partner's contracts).
    pub fee_min: String,
    pub fee_max: String,
    /// A contract begins / ends between these days (`YYYY-MM-DD`).
    pub start_from: String,
    pub start_to: String,
    pub end_from: String,
    pub end_to: String,
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
        if let Some(c) = self.location_id() {
            q.push(("location_id", c.to_string()));
        }
        if let Some(p) = parse_opt::<i32>(&self.provider_id).filter(|p| *p > 0) {
            q.push(("provider_id", p.to_string()));
        }
        for name in [
            "rating", "reply", "kind", "gap", "role_id", "area", "sector",
        ] {
            if let Some(v) = self.param(name) {
                q.push((name, v));
            }
        }
        if let Some(d) = self.created_from() {
            q.push(("from", d.to_string()));
        }
        if let Some(d) = self.created_to() {
            q.push(("to", d.to_string()));
        }
        if let Some(s) = self.search() {
            q.push(("q", s.to_owned()));
        }
        if let Some(s) = self.state() {
            q.push(("state", s.to_owned()));
        }
        if let Some(s) = self.sort() {
            q.push(("sort", s.code().to_owned()));
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

    /// One record's page (`route` with its `id`), or one of its actions,
    /// carrying this position: a record opened from a list keeps the way
    /// back to that page of it through its own forms and redirects.
    pub fn record_href(&self, route: &str, id: i32) -> Result<String, Error> {
        let query = self.query();
        let params: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        Ok(with_query(
            &url(route, &[("id", &id.to_string())])?,
            &params,
        ))
    }

    /// Page 1 of the whole list: no position worth keeping.
    pub fn is_default(&self) -> bool {
        self.query().is_empty()
    }

    /// 303 to one record's page, keeping this position, with `flash`.
    pub fn to_record(&self, route: &str, id: i32, flash: Flash) -> Result<Response, Error> {
        let mut response = see_other(&self.record_href(route, id)?)?;
        flash.set_on(&mut response);
        Ok(response)
    }

    /// 303 back to the list at `route`, with `flash`.
    pub fn back_to(&self, route: &str, flash: Flash) -> Result<Response, Error> {
        let mut response = see_other(&self.href(route, &[])?)?;
        flash.set_on(&mut response);
        Ok(response)
    }

    /// The first day asked for, when it is a date.
    pub fn created_from(&self) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::parse_from_str(self.from.trim(), "%Y-%m-%d").ok()
    }

    pub fn created_to(&self) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::parse_from_str(self.to.trim(), "%Y-%m-%d").ok()
    }

    pub fn category_id(&self) -> Option<i32> {
        parse_opt::<i32>(&self.category_id).filter(|c| *c > 0)
    }

    pub fn location_id(&self) -> Option<i32> {
        parse_opt::<i32>(&self.location_id).filter(|c| *c > 0)
    }

    pub fn provider_id(&self) -> Option<i32> {
        parse_opt::<i32>(&self.provider_id).filter(|p| *p > 0)
    }

    /// The star rating asked for, 1 to 5.
    pub fn rating(&self) -> Option<i16> {
        parse_opt::<i16>(&self.rating).filter(|n| (1..=5).contains(n))
    }

    /// Only what nobody has replied to yet.
    pub fn unanswered(&self) -> bool {
        self.reply.trim() == "none"
    }

    /// What the record is about (`kind`), when it is a code.
    pub fn kind(&self) -> Option<&str> {
        Some(self.kind.trim())
            .filter(|k| !k.is_empty() && k.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
    }

    /// A day typed (`YYYY-MM-DD`), when it is one.
    pub fn day(raw: &str) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").ok()
    }

    /// A fee typed, whole đồng, not negative.
    pub fn fee(&self, raw: &str) -> Option<i64> {
        parse_opt::<i64>(raw).filter(|n| *n >= 0)
    }

    /// The contract state asked for, when it is a code.
    pub fn contract(&self) -> Option<&str> {
        Some(self.contract.trim())
            .filter(|k| !k.is_empty() && k.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
    }

    /// The sector asked for, when it is a code.
    pub fn sector(&self) -> Option<&str> {
        Some(self.sector.trim())
            .filter(|k| !k.is_empty() && k.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
    }

    /// What a provider lacks (`gap`), when it is a code.
    pub fn gap(&self) -> Option<&str> {
        Some(self.gap.trim()).filter(|k| {
            !k.is_empty()
                && k.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        })
    }

    /// The address words typed, trimmed and cut at 60 characters.
    pub fn area(&self) -> Option<&str> {
        let a = self.area.trim();
        let a = a
            .char_indices()
            .nth(60)
            .map_or(a, |(at, _)| a[..at].trim_end());
        Some(a).filter(|a| !a.is_empty() && !a.chars().any(char::is_control))
    }

    /// The value of a criterion a toolbar select or toggle owns, as it goes
    /// in a URL: `None` when blank or not what the criterion takes.
    pub fn param(&self, name: &str) -> Option<String> {
        let id = |v: &str| {
            parse_opt::<i32>(v)
                .filter(|n| *n > 0)
                .map(|n| n.to_string())
        };
        let day = |v: &str| {
            chrono::NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d")
                .ok()
                .map(|d| d.to_string())
        };
        match name {
            "provider_id" => id(&self.provider_id),
            "crop_id" => id(&self.crop_id),
            "commodity_id" => id(&self.commodity_id),
            "category_id" => id(&self.category_id),
            "location_id" => id(&self.location_id),
            "role_id" => id(&self.role_id),
            "gap" => self.gap().map(str::to_owned),
            "sector" => self.sector().map(str::to_owned),
            "contract" => self.contract().map(str::to_owned),
            "fee_min" => self.fee(&self.fee_min).map(|n| n.to_string()),
            "fee_max" => self.fee(&self.fee_max).map(|n| n.to_string()),
            "start_from" => day(&self.start_from),
            "start_to" => day(&self.start_to),
            "end_from" => day(&self.end_from),
            "end_to" => day(&self.end_to),
            "area" => self.area().map(str::to_owned),
            "rating" => self.rating().map(|n| n.to_string()),
            "reply" => self.unanswered().then(|| "none".to_owned()),
            "kind" => self.kind().map(str::to_owned),
            _ => None,
        }
    }

    /// This position without the criterion `name`, back on page 1.
    pub fn without(&self, name: &str) -> Self {
        let mut p = self.clone();
        p.page = String::new();
        match name {
            "provider_id" => p.provider_id = String::new(),
            "crop_id" => p.crop_id = String::new(),
            "commodity_id" => p.commodity_id = String::new(),
            "category_id" => p.category_id = String::new(),
            "location_id" => p.location_id = String::new(),
            "rating" => p.rating = String::new(),
            "reply" => p.reply = String::new(),
            "kind" => p.kind = String::new(),
            "gap" => p.gap = String::new(),
            "sector" => p.sector = String::new(),
            "contract" => p.contract = String::new(),
            "fee_min" => p.fee_min = String::new(),
            "fee_max" => p.fee_max = String::new(),
            "start_from" => p.start_from = String::new(),
            "start_to" => p.start_to = String::new(),
            "end_from" => p.end_from = String::new(),
            "end_to" => p.end_to = String::new(),
            "role_id" => p.role_id = String::new(),
            "area" => p.area = String::new(),
            "from" => p.from = String::new(),
            "to" => p.to = String::new(),
            _ => {}
        }
        p
    }

    /// The text typed in the search box, trimmed and cut at
    /// `SEARCH_QUERY_MAX` characters; `None` when blank.
    pub fn search(&self) -> Option<&str> {
        let q = self.q.trim();
        let q = q
            .char_indices()
            .nth(SEARCH_QUERY_MAX)
            .map_or(q, |(at, _)| q[..at].trim_end());
        Some(q).filter(|q| !q.is_empty())
    }

    /// The page links of the list at `path`, keeping its provider filter and
    /// search.
    pub fn pagination(&self, path: &str, current: i64, total: i64) -> Option<Pagination> {
        let mut kept = self.query();
        kept.retain(|(k, _)| *k != "page");
        let params: Vec<(&str, &str)> = kept.iter().map(|(k, v)| (*k, v.as_str())).collect();
        Pagination::new(path, &params, current, total)
    }

    /// The order asked for, when it is one `AdminSort` knows.
    pub fn sort(&self) -> Option<AdminSort> {
        AdminSort::parse(self.sort.trim())
    }

    /// The criteria of the list: this position's provider, search, order and
    /// state, the state only when it is one of `states` (the list's own).
    pub fn filter(&self, states: &[&'static str]) -> AdminFilter {
        AdminFilter {
            provider_id: self.provider_id(),
            category_id: self.category_id(),
            location_id: self.location_id(),
            rating: self.rating(),
            unanswered: self.unanswered(),
            kind: self.kind().map(str::to_owned),
            role_id: parse_opt::<i32>(&self.role_id).filter(|n| *n > 0),
            gap: self.gap().map(str::to_owned),
            area: self.area().map(str::to_owned),
            sector: self.sector().map(str::to_owned),
            contract: self.contract().map(str::to_owned),
            today: None,
            created_from: self.created_from(),
            created_to: self.created_to(),
            search: self.search().map(str::to_owned),
            state: self.state_in(states),
            sort: self.sort(),
        }
    }

    /// This position as the list shows it: the state only when it is one of
    /// `states`, the order only when it is one of `sorts`, so a link never
    /// carries what the list ignored.
    pub fn checked(self, states: &[&'static str], sorts: &[AdminSort]) -> Self {
        let state = self.state_in(states).unwrap_or_default().to_owned();
        let sort = self
            .sort()
            .filter(|s| sorts.contains(s))
            .map(|s| s.code().to_owned())
            .unwrap_or_default();
        Self {
            state,
            sort,
            ..self
        }
    }

    /// The search box above the list at `route` (`molecules/admin_search`):
    /// one GET form that keeps every other criterion of the list (provider,
    /// commodity, type, tab, state) as hidden fields and goes back to the
    /// first page; its way out clears only the words. Its order select is
    /// added by `SearchBox::sorted`.
    pub fn search_box(&self, route: &str) -> Result<SearchBox, Error> {
        self.search_box_in(route, &[])
    }

    /// `search_box` for a list that is also narrowed by parameters of its
    /// own (`extra`: the tab `type`), which the box and its links keep.
    pub fn search_box_in(
        &self,
        route: &str,
        extra: &[(&'static str, &str)],
    ) -> Result<SearchBox, Error> {
        self.search_box_at(&url(route, &[])?, extra)
    }

    /// `search_box_in` for a list at `path` (a record's page, whose address
    /// holds its id).
    pub fn search_box_at(
        &self,
        path: &str,
        extra: &[(&'static str, &str)],
    ) -> Result<SearchBox, Error> {
        let mut keep = self.query();
        // The order is the result bar's, so the toolbar sends it back.
        keep.retain(|(k, _)| !matches!(*k, "q" | "page" | "from" | "to"));
        let without_search = ListPosition {
            q: String::new(),
            page: String::new(),
            ..self.clone()
        };
        let mut found = SearchBox {
            action: path.to_owned(),
            q: self.search().unwrap_or_default().to_owned(),
            keep: keep
                .into_iter()
                .chain(extra.iter().map(|(k, v)| (*k, (*v).to_owned())))
                .map(|(name, value)| HiddenField { name, value })
                .collect(),
            clear_href: {
                let query = without_search.query();
                let mut params: Vec<(&str, &str)> =
                    query.iter().map(|(k, v)| (*k, v.as_str())).collect();
                params.extend_from_slice(extra);
                with_query(path, &params)
            },
            sorts: Vec::new(),
            sort_keep: Vec::new(),
            searchable: true,
            picks: Vec::new(),
            toggles: Vec::new(),
            texts: Vec::new(),
            ranges: Vec::new(),
            dated: false,
            from: String::new(),
            to: String::new(),
            active: Vec::new(),
            filters_on: 0,
            narrowed: self.search().is_some() || self.state().is_some(),
            reset_href: String::new(),
            extra: extra.iter().map(|(k, v)| (*k, (*v).to_owned())).collect(),
            position: self.clone(),
            cleared: Vec::new(),
        };
        found.refresh_reset();
        Ok(found)
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
        waiting: Option<(&str, i64)>,
    ) -> Result<Vec<AdminTab>, Error> {
        self.state_chips_at(&url(route, &[])?, statuses, all, word, waiting)
    }

    /// `state_chips` for a list at `path` (a record's page).
    pub fn state_chips_at(
        &self,
        path: &str,
        statuses: &[&'static str],
        all: String,
        word: impl Fn(&str) -> String,
        waiting: Option<(&str, i64)>,
    ) -> Result<Vec<AdminTab>, Error> {
        let current = self.state_in(statuses);
        let chip = |state: Option<&str>, label: String| {
            let at = ListPosition {
                page: String::new(),
                state: state.unwrap_or_default().to_owned(),
                ..self.clone()
            };
            let count = waiting
                .filter(|(code, _)| state == Some(*code))
                .map(|(_, n)| usize::try_from(n).unwrap_or(0));
            Ok(AdminTab {
                label,
                href: {
                    let query = at.query();
                    let params: Vec<(&str, &str)> =
                        query.iter().map(|(k, v)| (*k, v.as_str())).collect();
                    with_query(path, &params)
                },
                count,
                current: current == state,
            })
        };
        std::iter::once(chip(None, all))
            .chain(statuses.iter().map(|s| chip(Some(s), word(s))))
            .collect()
    }

    /// The state chips of a list whose states are the shared ones
    /// (`admin_list::*_LIST_STATES`), worded by `admin_state.<code>`.
    pub fn shared_state_chips(
        &self,
        locale: &Locale,
        route: &str,
        statuses: &[&'static str],
        waiting: Option<(&str, i64)>,
    ) -> Result<Vec<AdminTab>, Error> {
        self.state_chips(
            route,
            statuses,
            locale.text("filters.all"),
            |s| locale.text(&format!("admin_state.{s}")),
            waiting,
        )
    }
}

/// The toolbar above an admin list (`molecules/admin_search`): one GET form
/// holding the search words, the status chips, the filters behind a button
/// (selects, toggles, a date range) and under them the criteria in force,
/// each removable. The order is not a filter: it is chosen on the result bar
/// under the toolbar (`molecules/admin_results`), a form of its own that
/// sends back every criterion. The toolbar keeps every other criterion of
/// the list as hidden fields and starts again at the first page.
#[derive(Debug, Serialize)]
pub struct SearchBox {
    pub action: String,
    /// The words box is shown (a list with nothing to search has none).
    pub searchable: bool,
    pub q: String,
    pub keep: Vec<HiddenField>,
    pub clear_href: String,
    /// The orders of the result bar's select; empty: the list has one.
    pub sorts: Vec<SortOption>,
    /// What the result bar's form sends back: every criterion but the order
    /// and the page.
    pub sort_keep: Vec<HiddenField>,
    /// Selects narrowing the list to one record of another kind.
    pub picks: Vec<PickField>,
    /// Checkboxes narrowing the list to what is in a state ("no reply yet").
    pub toggles: Vec<ToggleField>,
    /// Boxes for words (an address): sent when left.
    pub texts: Vec<TextField>,
    /// Number ranges (a fee): two boxes, sent when left.
    pub ranges: Vec<RangeField>,
    /// The date range is offered, and its values.
    pub dated: bool,
    pub from: String,
    pub to: String,
    /// The criteria in force, each with the list without it.
    pub active: Vec<ActiveFilter>,
    /// How many of the filters behind the button are on; the panel starts
    /// open when there is one.
    pub filters_on: usize,
    /// The words, a state chip or a filter narrow the list: the bar offers
    /// it back without them.
    pub narrowed: bool,
    /// The list without words, state, filters or order, first page, in what
    /// it is narrowed to (a tab, a type).
    pub reset_href: String,
    #[serde(skip)]
    extra: Vec<(&'static str, String)>,
    #[serde(skip)]
    position: ListPosition,
    /// The criteria the reset link drops besides the bar's own.
    #[serde(skip)]
    cleared: Vec<&'static str>,
}

/// A select of the toolbar: one record of another kind (an area, a service
/// type) as the query parameter `name`.
#[derive(Debug, Serialize)]
pub struct PickField {
    pub name: &'static str,
    pub label: String,
    pub all: String,
    pub options: Vec<PickOption>,
}

#[derive(Debug, Serialize)]
pub struct PickOption {
    pub value: String,
    pub label: String,
    pub selected: bool,
}

/// Two boxes of the toolbar for a number range: `min_name` and `max_name`.
#[derive(Debug, Serialize)]
pub struct RangeField {
    pub min_name: &'static str,
    pub max_name: &'static str,
    pub label: String,
    pub min: String,
    pub max: String,
    pub from_hint: String,
    pub to_hint: String,
    /// The boxes take days, not numbers.
    pub date: bool,
}

/// A box of the toolbar for words, as the query parameter `name`.
#[derive(Debug, Serialize)]
pub struct TextField {
    pub name: &'static str,
    pub label: String,
    pub placeholder: String,
    pub value: String,
}

/// A checkbox of the toolbar: `name=value` when ticked.
#[derive(Debug, Serialize)]
pub struct ToggleField {
    pub name: &'static str,
    pub value: &'static str,
    pub label: String,
    pub on: bool,
}

/// A criterion in force: its words, the list without it, and where the
/// words lead (the record the list is narrowed to), if anywhere.
#[derive(Debug, Serialize)]
pub struct ActiveFilter {
    pub label: String,
    pub href: String,
    pub link: Option<String>,
}

impl SearchBox {
    /// A words box alone, submitting to `action`, with nothing else to keep
    /// (a search inside a record's page).
    pub fn plain(action: String, q: &str) -> Self {
        Self {
            clear_href: action.clone(),
            reset_href: action.clone(),
            action,
            searchable: true,
            q: q.to_owned(),
            keep: Vec::new(),
            sorts: Vec::new(),
            sort_keep: Vec::new(),
            picks: Vec::new(),
            toggles: Vec::new(),
            texts: Vec::new(),
            ranges: Vec::new(),
            dated: false,
            from: String::new(),
            to: String::new(),
            active: Vec::new(),
            filters_on: 0,
            narrowed: false,
            extra: Vec::new(),
            position: ListPosition::default(),
            cleared: Vec::new(),
        }
    }

    /// The list at this box's path with `position` and the box's own extra
    /// parameters.
    fn at(&self, position: &ListPosition) -> String {
        let query = position.query();
        let mut params: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        params.extend(self.extra.iter().map(|(k, v)| (*k, v.as_str())));
        with_query(&self.action, &params)
    }

    /// The reset link: the list without words, state, order, dates and every
    /// criterion the bar owns; what the list is narrowed to (a tab, a type)
    /// stays, its own bar clears that.
    fn refresh_reset(&mut self) {
        let mut reset = ListPosition {
            q: String::new(),
            state: String::new(),
            sort: String::new(),
            from: String::new(),
            to: String::new(),
            page: String::new(),
            ..self.position.clone()
        };
        for name in &self.cleared {
            reset = reset.without(name);
        }
        self.reset_href = self.at(&reset);
    }

    /// The toolbar without the words box: a list with nothing to search.
    pub fn without_words(mut self) -> Self {
        self.searchable = false;
        self
    }

    /// Adds the date range (made from / to) and its removable chips.
    pub fn dated(mut self, locale: &Locale) -> Self {
        self.dated = true;
        if let Some(d) = self.position.created_from() {
            self.from = d.to_string();
            let href = self.at(&self.position.without("from"));
            self.active.push(ActiveFilter {
                label: locale.text_with(
                    "admin_search.from_chip",
                    &[("date", &d.format("%d/%m/%Y").to_string())],
                ),
                href,
                link: None,
            });
            self.narrowed = true;
            self.filters_on += 1;
        }
        if let Some(d) = self.position.created_to() {
            self.to = d.to_string();
            let href = self.at(&self.position.without("to"));
            self.active.push(ActiveFilter {
                label: locale.text_with(
                    "admin_search.to_chip",
                    &[("date", &d.format("%d/%m/%Y").to_string())],
                ),
                href,
                link: None,
            });
            self.narrowed = true;
            self.filters_on += 1;
        }
        self.refresh_reset();
        self
    }

    /// Adds a select choosing one of `options` (value, name) as the query
    /// parameter `name` (one `ListPosition::param` knows); the one chosen
    /// is the position's. `label` / `all`: its caption and its "any" option.
    pub fn picked<V: ToString>(
        mut self,
        name: &'static str,
        label: String,
        all: String,
        options: Vec<(V, String)>,
    ) -> Self {
        let chosen = self.position.param(name);
        let options: Vec<(String, String)> = options
            .into_iter()
            .map(|(v, l)| (v.to_string(), l))
            .collect();
        self.keep.retain(|f| f.name != name);
        self.cleared.push(name);
        if let Some((_, found)) = options.iter().find(|(v, _)| Some(v) == chosen.as_ref()) {
            let href = self.at(&self.position.without(name));
            self.active.push(ActiveFilter {
                label: format!("{label}: {found}"),
                href,
                link: None,
            });
            self.narrowed = true;
            self.filters_on += 1;
        }
        self.picks.push(PickField {
            name,
            label,
            all,
            options: options
                .into_iter()
                .map(|(value, label)| PickOption {
                    selected: chosen.as_deref() == Some(value.as_str()),
                    value,
                    label,
                })
                .collect(),
        });
        self.refresh_reset();
        self
    }

    /// Adds a box for words as the query parameter `name`.
    pub fn texted(mut self, name: &'static str, label: String, placeholder: String) -> Self {
        let value = self.position.param(name).unwrap_or_default();
        self.keep.retain(|f| f.name != name);
        self.cleared.push(name);
        if !value.is_empty() {
            let href = self.at(&self.position.without(name));
            self.active.push(ActiveFilter {
                label: format!("{label}: {value}"),
                href,
                link: None,
            });
            self.narrowed = true;
            self.filters_on += 1;
        }
        self.texts.push(TextField {
            name,
            label,
            placeholder,
            value,
        });
        self.refresh_reset();
        self
    }

    /// Adds a number range as the query parameters `min_name` and `max_name`.
    pub fn ranged(
        mut self,
        (min_name, max_name): (&'static str, &'static str),
        label: String,
        (from_hint, to_hint): (String, String),
    ) -> Self {
        let min = self.position.param(min_name).unwrap_or_default();
        let max = self.position.param(max_name).unwrap_or_default();
        self.keep
            .retain(|f| f.name != min_name && f.name != max_name);
        self.cleared.extend([min_name, max_name]);
        for (name, value, hint) in [(min_name, &min, &from_hint), (max_name, &max, &to_hint)] {
            if !value.is_empty() {
                let href = self.at(&self.position.without(name));
                self.active.push(ActiveFilter {
                    label: format!("{label}: {hint} {value}"),
                    href,
                    link: None,
                });
                self.narrowed = true;
                self.filters_on += 1;
            }
        }
        self.ranges.push(RangeField {
            min_name,
            max_name,
            label,
            min,
            max,
            from_hint,
            to_hint,
            date: false,
        });
        self.refresh_reset();
        self
    }

    /// Adds a day range as the query parameters `from_name` and `to_name`
    /// (`label`: what is dated, "Begins").
    pub fn date_ranged(
        mut self,
        locale: &Locale,
        (from_name, to_name): (&'static str, &'static str),
        label: String,
    ) -> Self {
        let from = self.position.param(from_name).unwrap_or_default();
        let to = self.position.param(to_name).unwrap_or_default();
        self.keep
            .retain(|f| f.name != from_name && f.name != to_name);
        self.cleared.extend([from_name, to_name]);
        let (from_hint, to_hint) = (
            locale.text("admin_search.from"),
            locale.text("admin_search.to"),
        );
        for (name, value, hint) in [(from_name, &from, &from_hint), (to_name, &to, &to_hint)] {
            if let Some(d) = ListPosition::day(value) {
                let href = self.at(&self.position.without(name));
                self.active.push(ActiveFilter {
                    label: format!("{label}: {hint} {}", d.format("%d/%m/%Y")),
                    href,
                    link: None,
                });
                self.narrowed = true;
                self.filters_on += 1;
            }
        }
        self.ranges.push(RangeField {
            min_name: from_name,
            max_name: to_name,
            label,
            min: from,
            max: to,
            from_hint,
            to_hint,
            date: true,
        });
        self.refresh_reset();
        self
    }

    /// Adds a checkbox: ticked, the list is narrowed by `name=value`.
    pub fn toggled(mut self, name: &'static str, value: &'static str, label: String) -> Self {
        let on = self.position.param(name).as_deref() == Some(value);
        self.keep.retain(|f| f.name != name);
        self.cleared.push(name);
        if on {
            let href = self.at(&self.position.without(name));
            self.active.push(ActiveFilter {
                label: label.clone(),
                href,
                link: None,
            });
            self.narrowed = true;
            self.filters_on += 1;
        }
        self.toggles.push(ToggleField {
            name,
            value,
            label,
            on,
        });
        self.refresh_reset();
        self
    }

    /// Adds the select of how long a waiting record may have waited (a
    /// review not shown, a new request, an open post).
    pub fn waiting(self, locale: &Locale) -> Self {
        self.picked(
            "gap",
            locale.text("admin_pick.wait"),
            locale.text("admin_pick.all_waits"),
            ["stale_3", "stale_7"]
                .into_iter()
                .map(|g| (g, locale.text(&format!("admin_pick.{g}"))))
                .collect(),
        )
    }

    /// The one provider the list is narrowed to, as the first criterion in
    /// force: its name leads to its page, its cross to the whole list.
    /// `label` words it when the list has no row to take the name from.
    pub fn provider(mut self, filter: Option<ProviderFilter>, label: String) -> Self {
        let Some(filter) = filter else {
            return self;
        };
        let href = self.at(&self.position.without("provider_id"));
        self.keep.retain(|f| f.name != "provider_id");
        self.cleared.push("provider_id");
        self.active.insert(
            0,
            ActiveFilter {
                label: filter.name.unwrap_or(label),
                href,
                link: Some(filter.href),
            },
        );
        self.narrowed = true;
        self.refresh_reset();
        self
    }

    /// The result bar's order select: the list's own order first (worded by
    /// `default_key`), then each of `sorts`, the one of `position` chosen.
    pub fn sorted(
        mut self,
        locale: &Locale,
        position: &ListPosition,
        default_key: &str,
        sorts: &[AdminSort],
    ) -> Self {
        let chosen = position.sort().filter(|s| sorts.contains(s));
        self.sorts = std::iter::once(SortOption {
            value: "",
            label: locale.text(default_key),
            selected: chosen.is_none(),
        })
        .chain(sorts.iter().map(|s| SortOption {
            value: s.code(),
            label: locale.text(&format!("admin_sort.{}", s.code())),
            selected: chosen == Some(*s),
        }))
        .collect();
        self.sort_keep = self
            .position
            .query()
            .into_iter()
            .filter(|(k, _)| !matches!(*k, "sort" | "page"))
            .chain(self.extra.iter().map(|(k, v)| (*k, v.clone())))
            .map(|(name, value)| HiddenField { name, value })
            .collect();
        self
    }
}

/// A criterion the search bar sends back unchanged.
#[derive(Debug, Serialize)]
pub struct HiddenField {
    pub name: &'static str,
    pub value: String,
}

/// One order of the search bar's select (`value` blank: the list's own).
#[derive(Debug, Serialize)]
pub struct SortOption {
    pub value: &'static str,
    pub label: String,
    pub selected: bool,
}

/// The one provider an admin list is narrowed to: its name, its admin page,
/// and the whole list.
#[derive(Debug, Clone, Serialize)]
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
    fn a_toolbar_criterion_leaves_the_others_when_removed() {
        let p = ListPosition {
            provider_id: "7".into(),
            rating: "4".into(),
            reply: "none".into(),
            page: "3".into(),
            ..ListPosition::default()
        };
        assert_eq!(
            p.query(),
            vec![
                ("provider_id", "7".to_owned()),
                ("rating", "4".to_owned()),
                ("reply", "none".to_owned()),
                ("page", "3".to_owned()),
            ]
        );
        assert_eq!(
            p.without("rating").query(),
            vec![
                ("provider_id", "7".to_owned()),
                ("reply", "none".to_owned())
            ],
            "back on the first page"
        );
        let bad = ListPosition {
            rating: "9".into(),
            reply: "x".into(),
            kind: "A b".into(),
            ..ListPosition::default()
        };
        assert!(bad.query().is_empty(), "what the list does not take");
    }

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
