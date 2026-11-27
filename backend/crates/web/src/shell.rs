//! The frame around every app page: top bar with search and the account menu,
//! and the bottom tab bar. Built on the server from who is asking, so the
//! right links are there before any script runs.
//!
//! The account links follow permissions, never role names: they only decide
//! which links to show. Every page still checks access itself.

use std::sync::Arc;

use application::{
    authz::Actor,
    catalogue::{
        PERM_ADMIN_DASHBOARD_VIEW, PERM_FEATURE_PREVIEW, PERM_OWN_LISTING_CREATE,
        PERM_PARTNER_REPORT_READ,
    },
    features::{Feature, FeaturesShown, Place, Stage},
    outputs::CodedDto,
    settings::{SettingGroup, Settings},
};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap},
};
use domain::Error;
use serde::Serialize;

use crate::{
    app_state::AppState,
    cookies::request_cookie,
    features::feature_of,
    labels::sector_options,
    locale::{Lang, Locale, Locales},
    middleware::{
        auth::Viewer,
        language::{preferred_language, LANG_COOKIE},
        page_settings,
    },
    pages::sector::chosen_sector,
    routes,
};

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
    /// Lit on a phone only: this tab stands in for the active page's own tab,
    /// which the phone's bar does not show (the shops directory is the second
    /// tab of the services list; the price board is reached from home).
    pub active_on_phone: bool,
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
    /// The footer's browse links: the pages the admin put there
    /// (`features::Place::Footer`) that this viewer sees.
    pub footer: Vec<Link>,
    /// "My area" as the last tab, kept to rebuild the tabs from the settings.
    #[serde(skip)]
    area_tab: Dest,
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
    /// Offered once to a browser that prefers another language we have.
    pub language_offer: Option<LanguageOffer>,
    /// The sector this visitor chose (`pages::sector`); `None`: every sector.
    pub sector: Option<String>,
    /// The sector switch's choices, "every sector" first, the chosen one
    /// marked; none while no sector is shown.
    pub sectors: Vec<SectorChoice>,
    /// The platform's own help lines in the footer (the `support.*`
    /// settings); empty when the settings could not be read.
    pub support: SupportLines,
    /// The features this viewer sees (`application::features`): a way in
    /// to a hidden one is not drawn. Templates read `shell.features.<code>`;
    /// page code `Shell::shows` and `Shell::shows_href`.
    pub features: FeaturesShown,
    /// This page is a feature in preview, seen by an admin: the strip
    /// under the top bar says farmers do not see it yet and links here,
    /// the settings' features tab.
    pub preview: Option<String>,
    /// The viewer holds `feature.preview`.
    #[serde(skip)]
    may_preview: bool,
}

/// The footer's help lines; one not set is not shown.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct SupportLines {
    pub phone: Option<String>,
    pub zalo_url: Option<String>,
    pub email: Option<String>,
}

impl SupportLines {
    pub fn of(settings: &Settings) -> Self {
        Self {
            phone: settings.support_phone().map(str::to_owned),
            zalo_url: settings.support_zalo_url().map(str::to_owned),
            email: settings.support_email().map(str::to_owned),
        }
    }
}

/// One button of the sector switch (`molecules/sector_switch.html`).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SectorChoice {
    /// The code, empty for every sector.
    pub value: String,
    pub label: String,
    pub icon: String,
    pub current: bool,
}

/// The icon of the switch's "every sector" button.
const EVERY_SECTOR_ICON: &str = "fa-border-all";

/// The switch's choices among the shown `sectors` for a visitor who chose
/// `sector`: none at all while there is no sector to choose.
pub fn sector_choices(
    locale: &Locale,
    sectors: &[CodedDto],
    sector: Option<&str>,
) -> Vec<SectorChoice> {
    if sectors.is_empty() {
        return Vec::new();
    }
    std::iter::once(SectorChoice {
        value: String::new(),
        label: locale.text("sector.all"),
        icon: EVERY_SECTOR_ICON.to_owned(),
        current: sector.is_none(),
    })
    .chain(sector_options(sectors).into_iter().map(|o| SectorChoice {
        current: sector == Some(o.value.as_str()),
        value: o.value,
        label: o.label,
        icon: o.icon,
    }))
    .collect()
}

/// "This site is in Vietnamese. Read it in English?" with its two answers,
/// each worded in its own language, since the visitor may read only one.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LanguageOffer {
    pub text: String,
    pub accept_code: &'static str,
    pub accept_label: String,
    pub stay_code: &'static str,
    pub stay_label: String,
}

/// The offer for a visitor who has not chosen a language and whose browser
/// prefers one of ours other than the default. Either answer sets the
/// cookie, so it shows once.
pub fn language_offer(headers: &HeaderMap, locales: &Locales) -> Option<LanguageOffer> {
    if request_cookie(headers, LANG_COOKIE).is_some() {
        return None;
    }
    let lang = preferred_language(headers).filter(|l| *l != Lang::DEFAULT)?;
    let theirs = locales.get(lang);
    Some(LanguageOffer {
        text: theirs.text("language.offer"),
        accept_code: lang.code(),
        accept_label: theirs.text(&format!("language.{}", lang.code())),
        stay_code: Lang::DEFAULT.code(),
        stay_label: locales.get(Lang::DEFAULT).text("language.stay"),
    })
}

fn url(name: &str) -> String {
    // The names are constants of this module and covered by its tests; an
    // unknown one falls back to home rather than failing the whole page.
    routes::url(name, &[]).unwrap_or_else(|_| "/".to_owned())
}

/// A link before it is worded: route name, icon and the label's text key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
const PARTNER_AREA: Dest = dest("partner_report", "fa-handshake", "account_menu.partner");
const ACCOUNT: Dest = dest("account", "fa-circle-user", "nav.account");

/// Whose area a signed-in visitor works in, decided by permissions (never a
/// role name): it picks the "my area" link and the frame of the pages every
/// account shares (the profile, an error page).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Admin,
    Partner,
    Provider,
    /// Anonymous, or an account with none of the above.
    Visitor,
}

impl Area {
    pub fn of(actor: Option<&Actor>) -> Self {
        match actor {
            Some(a) if a.can(PERM_ADMIN_DASHBOARD_VIEW) => Self::Admin,
            Some(a) if a.can(PERM_PARTNER_REPORT_READ) => Self::Partner,
            Some(a) if a.can(PERM_OWN_LISTING_CREATE) => Self::Provider,
            _ => Self::Visitor,
        }
    }

    fn dest(self) -> Dest {
        match self {
            Self::Admin => ADMIN_AREA,
            Self::Partner => PARTNER_AREA,
            Self::Provider => PROVIDER_AREA,
            Self::Visitor => ACCOUNT,
        }
    }

    /// Whether `path` is a page of this area: the admin panel (and the demo
    /// data page it frames), the partner's report, the provider's pages. An
    /// error there is shown in the area's frame, with the way back to it.
    pub fn covers(self, path: &str) -> bool {
        let under = |route: &str| {
            let base = url(route);
            path == base || path.starts_with(&format!("{base}/"))
        };
        match self {
            Self::Admin => under("admin") || under("setup_seed"),
            Self::Partner => under(PARTNER_AREA.route),
            Self::Provider => {
                path.starts_with(&section_of(PROVIDER_AREA.route, &url(PROVIDER_AREA.route)))
            }
            Self::Visitor => false,
        }
    }
}

/// Where "my area" is for this visitor: the admin panel, the partner's
/// report, the provider dashboard, or the account page (which asks anonymous
/// visitors to log in).
fn area_of(actor: Option<&Actor>) -> Dest {
    Area::of(actor).dest()
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
    match Area::of(actor) {
        Area::Admin => ACCOUNT,
        area => area.dest(),
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
        Some(_) if Area::of(actor) == Area::Provider => (
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

/// The section a tab covers. The provider's tab leads to the dashboard but
/// lights on every page of the provider area (inbox, jobs, reviews, forms),
/// which all live under the dashboard's parent path.
fn section_of(route: &str, href: &str) -> String {
    match href.rsplit_once('/') {
        Some((parent, _)) if route == PROVIDER_AREA.route => format!("{parent}/"),
        _ => href.to_owned(),
    }
}

impl Shell {
    pub fn new(locale: &Locale, path: &str, query: Option<&str>, actor: Option<&Actor>) -> Self {
        let mut shell = Self::without_session(locale, path, query);
        shell.may_preview = actor.is_some_and(|a| a.can(PERM_FEATURE_PREVIEW));
        shell.area_tab = area_tab(actor);
        shell.tabs = tabs(locale, path, shell.area_tab, &default_places);
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

    /// What the settings change in the frame: the footer's help lines, the
    /// tabs and the footer's links where the admin put each feature (never
    /// one this viewer does not see), the preview strip, and no invitation
    /// to sign up while sign-ups are closed.
    pub fn apply_settings(&mut self, locale: &Locale, settings: &Settings) {
        self.support = SupportLines::of(settings);
        self.features = settings.features_shown(self.may_preview);
        let features = self.features;
        let placed = |f: Feature, p: Place| features.has(f) && settings.placed(f, p);
        self.tabs = tabs(locale, &self.path, self.area_tab, &placed);
        self.footer = footer_links(locale, &placed);
        let (path, query) = self.here.split_once('?').unwrap_or((&self.here, ""));
        self.preview = feature_of(path, Some(query))
            .is_some_and(|f| settings.stage(f) == Stage::Preview && features.has(f))
            .then(|| {
                format!(
                    "{}?tab={}",
                    url("admin_settings"),
                    SettingGroup::Features.code()
                )
            });
        if !settings.registration_open() {
            let register = url("register");
            if let Some(menu) = self.profile.as_mut() {
                menu.items.retain(|item| item.href != register);
            }
            if self
                .provider_cta
                .as_ref()
                .is_some_and(|c| c.link.href == register)
            {
                self.provider_cta = None;
            }
        }
    }

    /// This viewer holds `feature.preview`: what is in preview shows to it.
    pub fn may_preview(&self) -> bool {
        self.may_preview
    }

    /// This viewer sees `feature`.
    pub fn shows(&self, feature: Feature) -> bool {
        self.features.has(feature)
    }

    /// A link to `href` may be drawn for this viewer: it leads to no
    /// feature it does not see.
    pub fn shows_href(&self, href: &str) -> bool {
        shown_href(self.features, href)
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
            language_offer: None,
            sector: None,
            sectors: Vec::new(),
            support: SupportLines::default(),
            features: FeaturesShown::ALL,
            preview: None,
            may_preview: false,
            tabs: tabs(locale, path, area_of(None), &default_places),
            footer: footer_links(locale, &default_places),
            area_tab: area_of(None),
            profile: None,
            provider_cta: None,
            search_query,
            search_url,
        }
    }
}

/// The places of a frame built before the settings are read: the defaults.
fn default_places(feature: Feature, place: Place) -> bool {
    application::features::default_placed(feature, place)
}

/// `href` (a path, maybe with its query) leads to no feature hidden from
/// whoever sees `features`.
fn shown_href(features: FeaturesShown, href: &str) -> bool {
    let (path, query) = href.split_once('?').unwrap_or((href, ""));
    feature_of(path, Some(query)).is_none_or(|f| features.has(f))
}

/// A page the frame leads to: its link, the feature it belongs to (`None`:
/// always on both bars and in the footer) and the tab a phone lights in its
/// stead when it is not on the phone's bar.
struct Browse {
    dest: Dest,
    feature: Option<Feature>,
    stand_in: &'static str,
}

/// Every page the frame may lead to, in the bars' and the footer's order.
const BROWSE: [Browse; 7] = [
    Browse {
        dest: dest("home", "fa-house", "nav.home"),
        feature: None,
        stand_in: "home",
    },
    Browse {
        dest: dest("explore", "fa-location-dot", "nav.near_me"),
        feature: Some(Feature::NearMe),
        stand_in: "home",
    },
    Browse {
        dest: dest("services", "fa-border-all", "nav.services"),
        feature: None,
        stand_in: "services",
    },
    Browse {
        dest: dest("products", "fa-bag-shopping", "nav.products"),
        feature: Some(Feature::Supplies),
        stand_in: "home",
    },
    // On a phone the directory is the second tab of /services.
    Browse {
        dest: dest("providers", "fa-store", "nav.shops"),
        feature: Some(Feature::Directory),
        stand_in: "services",
    },
    // On a phone the home page's price strip leads to the prices.
    Browse {
        dest: dest("prices", "fa-chart-line", "nav.prices"),
        feature: Some(Feature::Prices),
        stand_in: "home",
    },
    Browse {
        dest: dest("job_new", "fa-bullhorn", "nav.jobs"),
        feature: Some(Feature::Jobs),
        stand_in: "home",
    },
];

/// The pages the frame may lead to, worded, with the feature each belongs
/// to (`None`: always there), then the phone's account tab: what the
/// settings' preview of the frame draws.
pub fn browse_pages(locale: &Locale) -> (Vec<(Option<Feature>, Link)>, Link) {
    let pages = BROWSE
        .iter()
        .map(|b| (b.feature, b.dest.link(locale)))
        .collect();
    (pages, ACCOUNT.link(locale))
}

/// Whether a browse page is at `place`, by `placed` (the admin's places of
/// the features this viewer sees).
fn at(b: &Browse, place: Place, placed: &impl Fn(Feature, Place) -> bool) -> bool {
    b.feature.is_none_or(|f| placed(f, place))
}

/// Each browse page's places (`Place::ALL` order), never more features on a
/// bar than its room (`Place::room`), the first in the menu's order kept:
/// the settings refuse a save that overfills a bar, but one stored before
/// the rule must not overflow the frame either.
pub fn placed_pages(placed: &impl Fn(Feature, Place) -> bool) -> Vec<[bool; 3]> {
    let mut taken = [0usize; 3];
    BROWSE
        .iter()
        .map(|b| {
            let mut row = [false; 3];
            for (i, place) in Place::ALL.into_iter().enumerate() {
                let fits = b.feature.is_none()
                    || place
                        .room()
                        .is_none_or(|room| taken.get(i).is_some_and(|n| *n < room));
                let here = at(b, place, placed) && fits;
                if here && b.feature.is_some() {
                    if let Some(n) = taken.get_mut(i) {
                        *n += 1;
                    }
                }
                if let Some(slot) = row.get_mut(i) {
                    *slot = here;
                }
            }
            row
        })
        .collect()
}

/// The bars' tabs: each browse page on the wide bar, the phone's or both,
/// then "my area" (`area`).
fn tabs(
    locale: &Locale,
    path: &str,
    area: Dest,
    placed: &impl Fn(Feature, Place) -> bool,
) -> Vec<Tab> {
    let active = |d: &Dest| is_active(path, &section_of(d.route, &url(d.route)));
    let shown: Vec<(&Browse, bool, bool)> = BROWSE
        .iter()
        .zip(placed_pages(placed))
        .map(|(b, [top, phone, _])| (b, top, phone))
        .filter(|(_, top, phone)| *top || *phone)
        .collect();
    // An active page with no phone tab lights its stand-in there.
    let stand_in = shown
        .iter()
        .find(|(b, _, phone)| !phone && active(&b.dest))
        .map(|(b, ..)| b.stand_in);
    shown
        .iter()
        .map(|(b, top, phone)| Tab {
            active: active(&b.dest),
            active_on_phone: stand_in == Some(b.dest.route),
            wide_only: !phone,
            phone_only: !top,
            link: b.dest.link(locale),
        })
        .chain(std::iter::once(Tab {
            active: active(&area),
            active_on_phone: false,
            wide_only: false,
            phone_only: area.route == ACCOUNT.route,
            link: area.link(locale),
        }))
        .collect()
}

/// The footer's browse links: each browse page the admin put there.
fn footer_links(locale: &Locale, placed: &impl Fn(Feature, Place) -> bool) -> Vec<Link> {
    BROWSE
        .iter()
        .zip(placed_pages(placed))
        .filter(|(_, [.., footer])| *footer)
        .map(|(b, _)| b.dest.link(locale))
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
        let mut shell = Self::new(
            state.locale(),
            parts.uri.path(),
            parts.uri.query(),
            actor.as_ref(),
        );
        shell.language_offer = language_offer(&parts.headers, &state.locales);
        // The settings this request read already (`page_settings`), else
        // read now alongside the sectors.
        let read = async {
            match page_settings::current() {
                Some(settings) => Ok(settings),
                None => state.usecases.settings.current().await.map(Arc::new),
            }
        };
        let (sectors, settings) = tokio::try_join!(state.usecases.sector.shown(), read)?;
        shell.apply_settings(state.locale(), &settings);
        shell.sector = chosen_sector(&parts.headers, &sectors);
        shell.sectors = sector_choices(state.locale(), &sectors, shell.sector.as_deref());
        Ok(shell)
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
            name: "u".into(),
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
        assert_eq!(active("/supplies/category/4/phan-bon"), ["/supplies"]);
        assert_eq!(active("/providers/1/x"), ["/providers"]);
        assert_eq!(active("/services/3/phun-thuoc"), ["/services"]);
    }

    #[test]
    fn the_provider_tab_lights_on_every_page_of_the_provider_area() {
        let vi = vi();
        let provider = actor(&[PERM_OWN_LISTING_CREATE]);
        for path in [
            "/provider/dashboard",
            "/provider/inquiries",
            "/provider/jobs",
            "/provider/reviews",
            "/provider/services/3/edit",
        ] {
            let shell = Shell::new(&vi, path, None, provider.as_ref());
            let lit: Vec<_> = shell
                .tabs
                .iter()
                .filter(|t| t.active)
                .map(|t| t.link.href.as_str())
                .collect();
            assert_eq!(lit, ["/provider/dashboard"], "{path}");
        }
        // A public provider page belongs to the directory, not the dashboard.
        let shell = Shell::new(&vi, "/providers/1/x", None, provider.as_ref());
        assert!(!shell
            .tabs
            .iter()
            .any(|t| t.active && t.link.href == "/provider/dashboard"));
    }

    #[test]
    fn a_phone_lights_a_stand_in_for_a_wide_only_tab() {
        let vi = vi();
        let on_phone = |path: &str| {
            Shell::new(&vi, path, None, None)
                .tabs
                .iter()
                .filter(|t| (t.active && !t.wide_only) || t.active_on_phone)
                .map(|t| t.link.href.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(on_phone("/providers"), ["/services"]);
        assert_eq!(on_phone("/providers/1/x"), ["/services"]);
        assert_eq!(on_phone("/prices"), ["/"]);
        assert_eq!(on_phone("/services"), ["/services"]);
        assert_eq!(on_phone("/search"), Vec::<String>::new());
    }

    #[test]
    fn a_bar_never_holds_more_features_than_its_room() {
        // Every feature ticked everywhere, as a state stored before the rule.
        let rows = placed_pages(&|_, _| true);
        let features = |i: usize| {
            BROWSE
                .iter()
                .zip(&rows)
                .filter(|(b, row)| b.feature.is_some() && row.get(i).copied().unwrap_or(false))
                .count()
        };
        for (i, place) in Place::ALL.into_iter().enumerate() {
            match place.room() {
                Some(room) => assert_eq!(features(i), room, "{}", place.code()),
                None => assert_eq!(features(i), 5, "the footer holds every page"),
            }
        }
        // Home and the services are always on both bars and in the footer.
        for (b, row) in BROWSE.iter().zip(&rows) {
            if b.feature.is_none() {
                assert_eq!(*row, [true; 3], "{}", b.dest.route);
            }
        }
    }

    #[test]
    fn the_area_follows_permissions() {
        let admin = actor(&[PERM_ADMIN_DASHBOARD_VIEW, PERM_OWN_LISTING_CREATE]);
        assert_eq!(Area::of(admin.as_ref()), Area::Admin);
        let partner = actor(&[PERM_PARTNER_REPORT_READ]);
        assert_eq!(Area::of(partner.as_ref()), Area::Partner);
        let provider = actor(&[PERM_OWN_LISTING_CREATE]);
        assert_eq!(Area::of(provider.as_ref()), Area::Provider);
        assert_eq!(Area::of(actor(&[]).as_ref()), Area::Visitor);
        assert_eq!(Area::of(None), Area::Visitor);
    }

    #[test]
    fn each_area_covers_its_own_pages_only() {
        assert!(Area::Admin.covers("/admin/users/3"));
        assert!(Area::Admin.covers("/admin"));
        assert!(Area::Admin.covers("/setup/seed"));
        assert!(!Area::Admin.covers("/administration"));
        assert!(!Area::Admin.covers("/account"));
        assert!(Area::Partner.covers("/partner"));
        assert!(Area::Partner.covers("/partner/report.csv"));
        assert!(!Area::Partner.covers("/partners"));
        assert!(Area::Provider.covers("/provider/services/3/edit"));
        assert!(!Area::Provider.covers("/providers/1/x"));
        assert!(!Area::Visitor.covers("/"));
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
