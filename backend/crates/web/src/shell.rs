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
use domain::{Error, ErrorCode};
use serde::Serialize;

use crate::{app_state::AppState, middleware::auth::MaybeActor, routes};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Link {
    pub href: String,
    pub icon: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Tab {
    #[serde(flatten)]
    pub link: Link,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProfileMenu {
    pub signed_in: bool,
    pub items: Vec<Link>,
    /// Where the logout form posts; only when signed in.
    pub logout: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Shell {
    pub path: String,
    pub tabs: Vec<Tab>,
    /// `None` on error pages, which are rendered without knowing the session.
    pub profile: Option<ProfileMenu>,
    /// The `q` the user searched for, so the bar shows it after a search.
    pub search_query: String,
    pub search_url: String,
}

fn url(name: &str) -> String {
    // The names are constants of this module and covered by its tests; an
    // unknown one falls back to home rather than failing the whole page.
    routes::url(name, &[]).unwrap_or_else(|_| "/".to_owned())
}

fn link(route: &str, icon: &'static str, label: &'static str) -> Link {
    Link {
        href: url(route),
        icon,
        label,
    }
}

/// Where "my area" is for this visitor: the admin panel, the provider
/// dashboard, or the account page (which asks anonymous visitors to log in).
fn home_area(actor: Option<&Actor>) -> Link {
    match actor {
        Some(a) if a.can(PERM_ADMIN_DASHBOARD_VIEW) => {
            link("admin_dashboard", "fa-shield-halved", "Quản trị")
        }
        Some(a) if a.can(PERM_OWN_LISTING_CREATE) => {
            link("provider_dashboard", "fa-store", "Quản lý")
        }
        _ => link("account", "fa-circle-user", "Tài khoản"),
    }
}

fn is_active(path: &str, href: &str) -> bool {
    if href == "/" {
        path == "/"
    } else {
        path.starts_with(href)
    }
}

impl Shell {
    pub fn new(path: &str, query: Option<&str>, actor: Option<&Actor>) -> Self {
        let mut shell = Self::without_session(path, query);
        shell.tabs = tabs(path, home_area(actor));
        shell.profile = Some(match actor {
            None => ProfileMenu {
                signed_in: false,
                items: vec![
                    link("login", "fa-right-to-bracket", "Đăng nhập"),
                    link("register", "fa-user-plus", "Đăng ký"),
                ],
                logout: None,
            },
            Some(a) => {
                let mut items = Vec::new();
                let area = home_area(Some(a));
                if area.href != url("account") {
                    items.push(area);
                }
                items.push(link("account", "fa-id-card", "Hồ sơ"));
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
    pub fn without_session(path: &str, query: Option<&str>) -> Self {
        let search_url = url("search");
        let search_query = if path == search_url {
            // The search page has its own search box.
            String::new()
        } else {
            query_param(query, "q").unwrap_or_default()
        };
        Self {
            path: path.to_owned(),
            tabs: tabs(path, home_area(None)),
            profile: None,
            search_query,
            search_url,
        }
    }
}

fn tabs(path: &str, area: Link) -> Vec<Tab> {
    [
        link("home", "fa-compass", "Khám phá"),
        link("explore", "fa-location-dot", "Gần tôi"),
        link("services", "fa-border-all", "Dịch vụ"),
        link("products", "fa-bag-shopping", "Sản phẩm"),
        area,
    ]
    .into_iter()
    .map(|link| Tab {
        active: is_active(path, &link.href),
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
        let actor = match MaybeActor::from_request_parts(parts, state).await {
            Ok(MaybeActor(actor)) => actor,
            Err(e) if e.code() == ErrorCode::InvalidToken => None,
            Err(e) => return Err(e),
        };
        Ok(Self::new(
            parts.uri.path(),
            parts.uri.query(),
            actor.as_ref(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use domain::models::user::ActorRecord;

    use super::*;

    fn actor(permissions: &[&str]) -> Option<Actor> {
        Actor::from_record(ActorRecord {
            user_id: 1,
            username: "u".into(),
            role_code: "whatever".into(),
            is_active: true,
            permissions: permissions.iter().map(|p| (*p).to_owned()).collect(),
        })
    }

    fn area(shell: &Shell) -> Option<&Link> {
        shell.tabs.last().map(|t| &t.link)
    }

    #[test]
    fn the_last_tab_follows_permissions_not_roles() {
        let admin = actor(&[PERM_ADMIN_DASHBOARD_VIEW]);
        let provider = actor(&[PERM_OWN_LISTING_CREATE]);
        let neither = actor(&[]);
        let href = |a: Option<&Actor>| area(&Shell::new("/", None, a)).map(|l| l.href.clone());
        assert_eq!(href(admin.as_ref()), Some("/admin/dashboard".into()));
        assert_eq!(href(provider.as_ref()), Some("/provider/dashboard".into()));
        assert_eq!(href(neither.as_ref()), Some("/account".into()));
        assert_eq!(href(None), Some("/account".into()));
    }

    #[test]
    fn exactly_one_tab_is_active_and_home_only_on_home() {
        let active = |path: &str| {
            Shell::new(path, None, None)
                .tabs
                .iter()
                .filter(|t| t.active)
                .map(|t| t.link.href.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(active("/"), ["/"]);
        assert_eq!(active("/products/category/4/phan-bon"), ["/products"]);
        assert_eq!(active("/providers/1/x"), Vec::<String>::new());
    }

    #[test]
    fn signed_in_menus_offer_logout_and_anonymous_ones_offer_login() {
        let signed_in = Shell::new("/", None, actor(&[]).as_ref()).profile;
        assert!(signed_in
            .as_ref()
            .is_some_and(|p| p.signed_in && p.logout.is_some()));
        let anonymous = Shell::new("/", None, None).profile;
        assert!(anonymous.as_ref().is_some_and(|p| !p.signed_in
            && p.logout.is_none()
            && p.items.iter().any(|l| l.href == "/login")));
        assert_eq!(Shell::without_session("/x", None).profile, None);
    }

    #[test]
    fn the_search_bar_keeps_the_query_except_on_the_search_page() {
        let q = Some("q=ph%C3%A2n+b%C3%B3n&page=2");
        assert_eq!(Shell::new("/products", q, None).search_query, "phân bón");
        assert_eq!(Shell::new("/search", q, None).search_query, "");
        assert_eq!(percent_decode("%zz%4"), "%zz%4");
    }
}
