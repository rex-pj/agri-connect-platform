//! The frame around every app page: top bar with search and the account menu,
//! and the bottom tab bar. Built on the server from who is asking, so the
//! right links are there before any script runs.
//!
//! The account links follow permissions, never role names: they only decide
//! which links to show. Every page still checks access itself.

use application::{
    authz::Actor,
    catalogue::{PERM_ADMIN_DASHBOARD_VIEW, PERM_OWN_LISTING_CREATE},
};
use axum::{extract::FromRequestParts, http::request::Parts};
use domain::Error;
use serde::Serialize;

use crate::{app_state::AppState, locale::Locale, middleware::auth::Viewer, routes};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Link {
    pub href: String,
    pub icon: &'static str,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Tab {
    #[serde(flatten)]
    pub link: Link,
    pub active: bool,
    /// Only on the wide top bar: a phone's tab bar has room for five.
    pub wide_only: bool,
    /// Only on a phone's tab bar: the wide top bar has the account button.
    pub phone_only: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProfileMenu {
    pub signed_in: bool,
    pub items: Vec<Link>,
    /// Where the logout form posts; only when signed in.
    pub logout: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProviderCta {
    #[serde(flatten)]
    pub link: Link,
    pub title: String,
    pub sub: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Shell {
    pub path: String,
    pub tabs: Vec<Tab>,
    /// `None` on error pages, which are rendered without knowing the session.
    pub profile: Option<ProfileMenu>,
    /// The invitation to sell here (anonymous visitors) or the way back to
    /// the dashboard (providers); `None` for everyone else.
    pub provider_cta: Option<ProviderCta>,
    /// The `q` the user searched for, so the bar shows it after a search.
    pub search_query: String,
    pub search_url: String,
    /// This page with its query, where the language switch comes back to.
    pub here: String,
}

fn url(name: &str) -> String {
    // The names are constants of this module and covered by its tests; an
    // unknown one falls back to home rather than failing the whole page.
    routes::url(name, &[]).unwrap_or_else(|_| "/".to_owned())
}

/// A link before it is worded: route name, icon and the label's text key.
#[derive(Debug, Clone, Copy)]
struct Dest {
    route: &'static str,
    icon: &'static str,
    label: &'static str,
}

const fn dest(route: &'static str, icon: &'static str, label: &'static str) -> Dest {
    Dest { route, icon, label }
}

impl Dest {
    fn link(self, locale: &Locale) -> Link {
        Link {
            href: url(self.route),
            icon: self.icon,
            label: locale.text(self.label),
        }
    }
}

const ADMIN_AREA: Dest = dest("admin_dashboard", "fa-shield-halved", "account_menu.admin");
const PROVIDER_AREA: Dest = dest("provider_dashboard", "fa-store", "account_menu.manage");
const ACCOUNT: Dest = dest("account", "fa-circle-user", "nav.account");

/// Where "my area" is for this visitor: the admin panel, the provider
/// dashboard, or the account page (which asks anonymous visitors to log in).
fn area_of(actor: Option<&Actor>) -> Dest {
    match actor {
        Some(a) if a.can(PERM_ADMIN_DASHBOARD_VIEW) => ADMIN_AREA,
        Some(a) if a.can(PERM_OWN_LISTING_CREATE) => PROVIDER_AREA,
        _ => ACCOUNT,
    }
}

/// "My area" as a worded link.
pub fn home_area(locale: &Locale, actor: Option<&Actor>) -> Link {
    area_of(actor).link(locale)
}

/// Where "my area" is, for a redirect.
pub fn home_area_href(actor: Option<&Actor>) -> String {
    url(area_of(actor).route)
}

/// The last tab. The admin panel is a back office, not a place to browse: it
/// is reached from the account menu and the footer, so admins get the plain
/// account tab here.
fn area_tab(actor: Option<&Actor>) -> Dest {
    match actor {
        Some(a) if a.can(PERM_ADMIN_DASHBOARD_VIEW) => ACCOUNT,
        _ => area_of(actor),
    }
}

/// Providers are otherwise invisible to visitors: the menu only says how to
/// sign up, so the home page invites them, and brings signed-in providers back
/// to their dashboard. Admins have their own panel and see nothing.
pub fn provider_cta(locale: &Locale, actor: Option<&Actor>) -> Option<ProviderCta> {
    let (target, title, sub) = match actor {
        None => (
            dest("register", "fa-store", "provider_cta.register"),
            "provider_cta.visitor_title",
            "provider_cta.visitor_sub",
        ),
        Some(a) if !a.can(PERM_ADMIN_DASHBOARD_VIEW) && a.can(PERM_OWN_LISTING_CREATE) => (
            PROVIDER_AREA,
            "provider_cta.provider_title",
            "provider_cta.provider_sub",
        ),
        Some(_) => return None,
    };
    Some(ProviderCta {
        link: target.link(locale),
        title: locale.text(title),
        sub: locale.text(sub),
    })
}

fn is_active(path: &str, href: &str) -> bool {
    if href == "/" {
        path == "/"
    } else {
        path.starts_with(href)
    }
}

impl Shell {
    pub fn new(locale: &Locale, path: &str, query: Option<&str>, actor: Option<&Actor>) -> Self {
        let mut shell = Self::without_session(locale, path, query);
        shell.tabs = tabs(locale, path, area_tab(actor));
        shell.provider_cta = provider_cta(locale, actor);
        shell.profile = Some(match actor {
            None => ProfileMenu {
                signed_in: false,
                items: vec![
                    dest("login", "fa-right-to-bracket", "account_menu.login").link(locale),
                    dest("register", "fa-store", "account_menu.register_provider").link(locale),
                ],
                logout: None,
            },
            Some(a) => {
                let mut items = Vec::new();
                let area = home_area(locale, Some(a));
                if area.href != url("account") {
                    items.push(area);
                }
                items.push(dest("account", "fa-id-card", "account_menu.profile").link(locale));
                ProfileMenu {
                    signed_in: true,
                    items,
                    logout: Some(url("logout")),
                }
            }
        });
        shell
    }

    /// For error pages: the frame and the generic account tab, no menu.
    pub fn without_session(locale: &Locale, path: &str, query: Option<&str>) -> Self {
        let search_url = url("search");
        let search_query = if path == search_url {
            // The search page has its own search box.
            String::new()
        } else {
            query_param(query, "q").unwrap_or_default()
        };
        let here = match query.filter(|q| !q.is_empty()) {
            Some(q) => format!("{path}?{q}"),
            None => path.to_owned(),
        };
        Self {
            path: path.to_owned(),
            here,
            tabs: tabs(locale, path, area_of(None)),
            profile: None,
            provider_cta: None,
            search_query,
            search_url,
        }
    }
}

fn tabs(locale: &Locale, path: &str, area: Dest) -> Vec<Tab> {
    [
        dest("home", "fa-house", "nav.home"),
        dest("explore", "fa-location-dot", "nav.near_me"),
        dest("services", "fa-border-all", "nav.services"),
        dest("products", "fa-bag-shopping", "nav.products"),
        // The provider directory; on a phone it is a home page tile.
        dest("providers", "fa-store", "nav.shops"),
        area,
    ]
    .into_iter()
    .map(|d| d.link(locale))
    .map(|link| Tab {
        active: is_active(path, &link.href),
        wide_only: link.href == url("providers"),
        phone_only: link.href == url("account"),
        link,
    })
    .collect()
}

/// `q` from a raw query string, `+` and `%XX` decoded.
fn query_param(query: Option<&str>, name: &str) -> Option<String> {
    query?
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| percent_decode(&v.replace('+', " ")))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (b, hex) {
            (b'%', Some(decoded)) => {
                out.push(decoded);
                i += 3;
            }
            _ => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The shell for the current request. An expired or forged session cookie
/// shows the page signed out instead of failing it: a stale cookie must never
/// make the home page unreachable.
impl FromRequestParts<AppState> for Shell {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Viewer(actor) = Viewer::from_request_parts(parts, state).await?;
        Ok(Self::new(
            state.locale(),
            parts.uri.path(),
            parts.uri.query(),
            actor.as_ref(),
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use domain::models::user::ActorRecord;

    use super::*;

    fn vi() -> Locale {
        Locale::vi().expect("vi.toml parses")
    }

    fn actor(permissions: &[&str]) -> Option<Actor> {
        Actor::from_record(ActorRecord {
            user_id: 1,
            username: "u".into(),
            role_code: "whatever".into(),
            is_active: true,
            permissions: permissions.iter().map(|p| (*p).to_owned()).collect(),
            sessions_valid_after: None,
        })
    }

    fn area(shell: &Shell) -> Option<&Link> {
        shell.tabs.last().map(|t| &t.link)
    }

    #[test]
    fn the_last_tab_follows_permissions_not_roles() {
        let vi = vi();
        let admin = actor(&[PERM_ADMIN_DASHBOARD_VIEW]);
        let provider = actor(&[PERM_OWN_LISTING_CREATE]);
        let neither = actor(&[]);
        let href = |a: Option<&Actor>| area(&Shell::new(&vi, "/", None, a)).map(|l| l.href.clone());
        assert_eq!(href(admin.as_ref()), Some("/account".into()));
        assert_eq!(href(provider.as_ref()), Some("/provider/dashboard".into()));
        assert_eq!(href(neither.as_ref()), Some("/account".into()));
        assert_eq!(href(None), Some("/account".into()));
        assert_eq!(home_area_href(provider.as_ref()), "/provider/dashboard");
        assert_eq!(home_area_href(admin.as_ref()), "/admin/dashboard");
    }

    #[test]
    fn the_admin_panel_is_in_the_account_menu_not_the_tabs() {
        let admin = actor(&[PERM_ADMIN_DASHBOARD_VIEW]);
        let shell = Shell::new(&vi(), "/", None, admin.as_ref());
        assert!(!shell.tabs.iter().any(|t| t.link.href == "/admin/dashboard"));
        // The account tab would repeat the top bar's account button.
        assert!(shell.tabs.last().is_some_and(|t| t.phone_only));
        assert!(shell
            .profile
            .is_some_and(|p| p.items.iter().any(|l| l.href == "/admin/dashboard")));
    }

    #[test]
    fn the_provider_invitation_goes_to_visitors_and_providers_only() {
        let vi = vi();
        let href = |a: Option<&Actor>| provider_cta(&vi, a).map(|c| c.link.href);
        assert_eq!(href(None), Some("/register".into()));
        let provider = actor(&[PERM_OWN_LISTING_CREATE]);
        assert_eq!(href(provider.as_ref()), Some("/provider/dashboard".into()));
        let admin = actor(&[PERM_ADMIN_DASHBOARD_VIEW, PERM_OWN_LISTING_CREATE]);
        assert_eq!(href(admin.as_ref()), None);
        assert_eq!(href(actor(&[]).as_ref()), None);
    }

    #[test]
    fn every_label_is_worded() {
        let vi = vi();
        let shell = Shell::new(&vi, "/", None, actor(&[PERM_OWN_LISTING_CREATE]).as_ref());
        let labels = shell
            .tabs
            .iter()
            .map(|t| &t.link)
            .chain(shell.profile.iter().flat_map(|p| p.items.iter()))
            .map(|l| l.label.as_str());
        for label in labels {
            assert!(
                !label.is_empty() && !label.contains('.'),
                "{label:?} has no text"
            );
        }
    }

    #[test]
    fn exactly_one_tab_is_active_and_home_only_on_home() {
        let vi = vi();
        let active = |path: &str| {
            Shell::new(&vi, path, None, None)
                .tabs
                .iter()
                .filter(|t| t.active)
                .map(|t| t.link.href.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(active("/"), ["/"]);
        assert_eq!(active("/products/category/4/phan-bon"), ["/products"]);
        assert_eq!(active("/providers/1/x"), ["/providers"]);
        assert_eq!(active("/services/3/phun-thuoc"), ["/services"]);
    }

    #[test]
    fn signed_in_menus_offer_logout_and_anonymous_ones_offer_login() {
        let vi = vi();
        let signed_in = Shell::new(&vi, "/", None, actor(&[]).as_ref()).profile;
        assert!(signed_in
            .as_ref()
            .is_some_and(|p| p.signed_in && p.logout.is_some()));
        let anonymous = Shell::new(&vi, "/", None, None).profile;
        assert!(anonymous.as_ref().is_some_and(|p| !p.signed_in
            && p.logout.is_none()
            && p.items.iter().any(|l| l.href == "/login")));
        assert_eq!(Shell::without_session(&vi, "/x", None).profile, None);
    }

    #[test]
    fn the_search_bar_keeps_the_query_except_on_the_search_page() {
        let vi = vi();
        let q = Some("q=ph%C3%A2n+b%C3%B3n&page=2");
        assert_eq!(
            Shell::new(&vi, "/products", q, None).search_query,
            "phân bón"
        );
        assert_eq!(Shell::new(&vi, "/search", q, None).search_query, "");
        assert_eq!(percent_decode("%zz%4"), "%zz%4");
    }
}
