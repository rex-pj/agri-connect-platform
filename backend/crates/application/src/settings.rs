//! The settings an admin changes without a deploy: the registry of every
//! key (its group, kind, default and bounds) and the typed view the use
//! cases read.
//!
//! Storage (`platform_settings`) keeps only the settings moved off their
//! defaults, as text. A key added here needs no migration; one removed here
//! leaves a row nothing reads (logged at load). The defaults are the
//! constants the code ran on before these were settings.
//!
//! Only policy that an admin may sensibly tune lives here. Secrets,
//! infrastructure, security limits (rate limits, lockout, password rules),
//! the privacy floor of the demand report and data retention stay in code
//! or the environment: an admin account taken over must not be able to
//! lower them.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use domain::{
    models::setting::StoredSetting, repositories::setting_repository_trait::SettingRepositoryTrait,
    Error, ErrorCode, Result,
};
use tracing::warn;
use uuid::Uuid;

use crate::{
    constants::{
        EMAIL_MAX, JOB_NOTIFY_MAX_DEFAULT, JOB_NOTIFY_RADIUS_KM_DEFAULT,
        JOB_POSTS_OPEN_PER_PHONE_DEFAULT, JOB_POSTS_PER_CLIENT_PER_DAY_DEFAULT,
        JOB_POST_DAYS_DEFAULT, JOB_POST_KEEP_DAYS, METRES_PER_KM, PRODUCTS_PER_BUSINESS_DEFAULT,
        PRODUCTS_PER_BUSINESS_MAX, RADIUS_MAX_M, REVIEW_SUSPECT_MINUTES_DEFAULT,
        SERVICES_PER_BUSINESS_DEFAULT, SERVICES_PER_BUSINESS_MAX, SITE_DESCRIPTION_MAX,
        SITE_NAME_MAX, SPONSORED_SHOWN_DEFAULT, SPONSORED_SHOWN_MAX, STEADY_WEEKLY_LEADS_DEFAULT,
    },
    features::{Feature, FeaturesShown, Place, Placement, Stage, PLACEMENTS},
    validation::{is_valid_vn_phone, max_chars, one_line, validate_email},
};

/// What a key looks like (`chk_platform_settings_key`): dotted lower-case
/// words, at least two.
pub const KEY_PATTERN: &str = "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+$";
/// The longest value stored (`platform_settings.value`).
pub const SETTING_VALUE_MAX: usize = 500;
/// A link an admin enters is opened from every page's footer: only these.
pub const URL_SCHEME: &str = "https://";
/// Stored text of a switch.
const ON: &str = "true";
const OFF: &str = "false";

// ── Keys ──────────────────────────────────────────────────────────────────────

pub const BRAND_NAME: &str = "brand.name";
pub const BRAND_DESCRIPTION: &str = "brand.description";
pub const BRAND_LOGO: &str = "brand.logo";
pub const BRAND_ICON: &str = "brand.icon";
pub const SUPPORT_PHONE: &str = "support.phone";
pub const SUPPORT_ZALO_URL: &str = "support.zalo_url";
pub const SUPPORT_EMAIL: &str = "support.email";
pub const REGISTRATION_OPEN: &str = "access.registration_open";
pub const JOB_POSTS_PER_CLIENT_PER_DAY: &str = "jobs.posts_per_client_per_day";
pub const JOB_POSTS_OPEN_PER_PHONE: &str = "jobs.open_per_phone";
pub const JOB_POST_DAYS: &str = "jobs.open_days";
pub const JOB_NOTIFY_RADIUS_KM: &str = "jobs.notify_radius_km";
pub const JOB_NOTIFY_MAX: &str = "jobs.notify_max";
pub const SERVICES_PER_BUSINESS: &str = "listings.services_per_business";
pub const PRODUCTS_PER_BUSINESS: &str = "listings.products_per_business";
pub const REVIEW_SUSPECT_MINUTES: &str = "listings.review_suspect_minutes";
pub const STEADY_WEEKLY_LEADS: &str = "market.steady_weekly_leads";
pub const SPONSORED_SHOWN: &str = "market.sponsored_shown";

/// The sections of the settings page, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingGroup {
    /// What the platform is called and looks like: every page's title, top
    /// bar, logo and browser icon; emails.
    Brand,
    /// The platform's own help lines in the footer.
    Support,
    /// What is open to the public: sign-ups and each feature's stage
    /// (`features`).
    Features,
    /// Job posts: how many, how long, who is told.
    Jobs,
    /// What a provider may list; how reviews are watched.
    Listings,
    /// The figures partners are sold on.
    Market,
}

impl SettingGroup {
    pub const ALL: [SettingGroup; 6] = [
        SettingGroup::Brand,
        SettingGroup::Support,
        SettingGroup::Features,
        SettingGroup::Jobs,
        SettingGroup::Listings,
        SettingGroup::Market,
    ];

    /// Its stable name: the page's anchor, the reset form's value and its
    /// locale keys (`settings.group.<code>`).
    pub const fn code(self) -> &'static str {
        match self {
            SettingGroup::Brand => "brand",
            SettingGroup::Support => "support",
            SettingGroup::Features => "features",
            SettingGroup::Jobs => "jobs",
            SettingGroup::Listings => "listings",
            SettingGroup::Market => "market",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.code() == code)
    }
}

/// What a text setting holds; empty is always allowed (the line not shown).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRule {
    /// A Vietnamese mobile (`validation::is_valid_vn_phone`).
    Phone,
    /// A link starting `https://`.
    HttpsUrl,
    Email,
    /// Plain words on one line (`validation::one_line`), at most `max`
    /// characters: it lands in page titles and email subjects.
    Line {
        max: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    /// A whole number from `min` to `max`.
    Number { default: i64, min: i64, max: i64 },
    /// On or off.
    Switch { default: bool },
    /// Who sees a feature: on, preview, off (`features::Stage`).
    Stage { default: Stage },
    /// Text, empty by default.
    Text(TextRule),
    /// An uploaded image (`files.id`), empty by default. Set only by an
    /// upload (`SettingsUseCase::set_image`), never typed.
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingDef {
    pub key: &'static str,
    pub group: SettingGroup,
    pub kind: SettingKind,
}

const fn number(
    key: &'static str,
    group: SettingGroup,
    default: i64,
    min: i64,
    max: i64,
) -> SettingDef {
    SettingDef {
        key,
        group,
        kind: SettingKind::Number { default, min, max },
    }
}

const fn stage(feature: Feature) -> SettingDef {
    SettingDef {
        key: feature.setting_key(),
        group: SettingGroup::Features,
        kind: SettingKind::Stage { default: Stage::On },
    }
}

/// The `i`-th of `PLACEMENTS` as a switch of the features tab (walked, not
/// indexed: an index past the end leaves an empty key, which
/// `keys_are_unique_dotted_and_fit_the_column` refuses).
const fn place(i: usize) -> SettingDef {
    let mut rest: &[Placement] = &PLACEMENTS;
    let mut n = i;
    let (mut key, mut default) = ("", false);
    while let [head, tail @ ..] = rest {
        if n == 0 {
            key = head.key;
            default = head.default;
            break;
        }
        n -= 1;
        rest = tail;
    }
    SettingDef {
        key,
        group: SettingGroup::Features,
        kind: SettingKind::Switch { default },
    }
}

/// Every setting, in the page's order within each group.
pub static SETTINGS: &[SettingDef] = &[
    // Empty: the deployment's own (`SITE_NAME`, `SITE_DESCRIPTION`).
    SettingDef {
        key: BRAND_NAME,
        group: SettingGroup::Brand,
        kind: SettingKind::Text(TextRule::Line { max: SITE_NAME_MAX }),
    },
    SettingDef {
        key: BRAND_DESCRIPTION,
        group: SettingGroup::Brand,
        kind: SettingKind::Text(TextRule::Line {
            max: SITE_DESCRIPTION_MAX,
        }),
    },
    // Empty: the built-in mark and icons.
    SettingDef {
        key: BRAND_LOGO,
        group: SettingGroup::Brand,
        kind: SettingKind::Image,
    },
    SettingDef {
        key: BRAND_ICON,
        group: SettingGroup::Brand,
        kind: SettingKind::Image,
    },
    SettingDef {
        key: SUPPORT_PHONE,
        group: SettingGroup::Support,
        kind: SettingKind::Text(TextRule::Phone),
    },
    SettingDef {
        key: SUPPORT_ZALO_URL,
        group: SettingGroup::Support,
        kind: SettingKind::Text(TextRule::HttpsUrl),
    },
    SettingDef {
        key: SUPPORT_EMAIL,
        group: SettingGroup::Support,
        kind: SettingKind::Text(TextRule::Email),
    },
    SettingDef {
        key: REGISTRATION_OPEN,
        group: SettingGroup::Features,
        kind: SettingKind::Switch { default: true },
    },
    // Every feature open by default: the platform ran with all of them.
    stage(Feature::Prices),
    stage(Feature::NearMe),
    stage(Feature::Supplies),
    stage(Feature::Directory),
    stage(Feature::Reviews),
    stage(Feature::Jobs),
    // Where each feature's page is led to from (`features::PLACEMENTS`).
    place(0),
    place(1),
    place(2),
    place(3),
    place(4),
    place(5),
    place(6),
    place(7),
    place(8),
    place(9),
    place(10),
    place(11),
    place(12),
    place(13),
    place(14),
    number(
        JOB_POSTS_PER_CLIENT_PER_DAY,
        SettingGroup::Jobs,
        JOB_POSTS_PER_CLIENT_PER_DAY_DEFAULT,
        1,
        50,
    ),
    number(
        JOB_POSTS_OPEN_PER_PHONE,
        SettingGroup::Jobs,
        JOB_POSTS_OPEN_PER_PHONE_DEFAULT,
        1,
        20,
    ),
    // Never past the day the post is deleted.
    number(
        JOB_POST_DAYS,
        SettingGroup::Jobs,
        JOB_POST_DAYS_DEFAULT,
        1,
        30,
    ),
    // The near-me radius bounds: 1 km to RADIUS_MAX_M.
    number(
        JOB_NOTIFY_RADIUS_KM,
        SettingGroup::Jobs,
        JOB_NOTIFY_RADIUS_KM_DEFAULT,
        1,
        (RADIUS_MAX_M as i64) / METRES_PER_KM,
    ),
    number(
        JOB_NOTIFY_MAX,
        SettingGroup::Jobs,
        JOB_NOTIFY_MAX_DEFAULT,
        1,
        100,
    ),
    number(
        SERVICES_PER_BUSINESS,
        SettingGroup::Listings,
        SERVICES_PER_BUSINESS_DEFAULT as i64,
        1,
        SERVICES_PER_BUSINESS_MAX as i64,
    ),
    number(
        PRODUCTS_PER_BUSINESS,
        SettingGroup::Listings,
        PRODUCTS_PER_BUSINESS_DEFAULT as i64,
        1,
        PRODUCTS_PER_BUSINESS_MAX as i64,
    ),
    number(
        REVIEW_SUSPECT_MINUTES,
        SettingGroup::Listings,
        REVIEW_SUSPECT_MINUTES_DEFAULT,
        0,
        24 * 60,
    ),
    number(
        STEADY_WEEKLY_LEADS,
        SettingGroup::Market,
        STEADY_WEEKLY_LEADS_DEFAULT,
        1,
        1_000,
    ),
    number(
        SPONSORED_SHOWN,
        SettingGroup::Market,
        SPONSORED_SHOWN_DEFAULT as i64,
        0,
        SPONSORED_SHOWN_MAX as i64,
    ),
];

// The bounds must keep each default and stay inside the code's own limits.
const _: () = assert!(JOB_POST_DAYS_DEFAULT <= JOB_POST_KEEP_DAYS);

/// The registry entry of `key`.
pub fn definition(key: &str) -> Option<&'static SettingDef> {
    SETTINGS.iter().find(|d| d.key == key)
}

/// A setting's value, typed by its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingValue {
    Number(i64),
    Switch(bool),
    Stage(Stage),
    /// Empty: not set.
    Text(String),
}

impl SettingValue {
    /// As stored and as a form box shows it.
    pub fn as_text(&self) -> String {
        match self {
            SettingValue::Number(n) => n.to_string(),
            SettingValue::Switch(true) => ON.to_owned(),
            SettingValue::Switch(false) => OFF.to_owned(),
            SettingValue::Stage(stage) => stage.code().to_owned(),
            SettingValue::Text(t) => t.clone(),
        }
    }
}

impl SettingDef {
    pub fn default_value(&self) -> SettingValue {
        match self.kind {
            SettingKind::Number { default, .. } => SettingValue::Number(default),
            SettingKind::Switch { default } => SettingValue::Switch(default),
            SettingKind::Stage { default } => SettingValue::Stage(default),
            SettingKind::Text(_) | SettingKind::Image => SettingValue::Text(String::new()),
        }
    }

    /// `raw` as this setting's value, or the coded reason it is not one
    /// (the caller files it under the key: `Validation::rule`).
    pub fn parse(&self, raw: &str) -> Result<SettingValue> {
        let raw = raw.trim();
        let refuse = |code: ErrorCode| Error::new(code);
        match self.kind {
            SettingKind::Number { min, max, .. } => {
                let n: i64 = raw.parse().map_err(|_| refuse(ErrorCode::InvalidNumber))?;
                if n < min || n > max {
                    return Err(Error::new(ErrorCode::SettingOutOfRange)
                        .with_param("min", min)
                        .with_param("max", max));
                }
                Ok(SettingValue::Number(n))
            }
            SettingKind::Switch { .. } => match raw {
                ON => Ok(SettingValue::Switch(true)),
                OFF | "" => Ok(SettingValue::Switch(false)),
                _ => Err(refuse(ErrorCode::InvalidRequestBody)),
            },
            SettingKind::Stage { .. } => Stage::parse(raw)
                .map(SettingValue::Stage)
                .ok_or_else(|| refuse(ErrorCode::InvalidRequestBody)),
            SettingKind::Text(_) | SettingKind::Image if raw.is_empty() => {
                Ok(SettingValue::Text(String::new()))
            }
            // Read back from storage only: an upload writes it.
            SettingKind::Image => Uuid::parse_str(raw)
                .map(|id| SettingValue::Text(id.to_string()))
                .map_err(|_| refuse(ErrorCode::InvalidRequestBody)),
            SettingKind::Text(TextRule::Line { max }) => {
                let line = one_line(raw);
                max_chars(&line, max)?;
                Ok(SettingValue::Text(line))
            }
            SettingKind::Text(TextRule::Phone) => {
                let digits: String = raw
                    .chars()
                    .filter(|c| !c.is_whitespace() && *c != '.')
                    .collect();
                if is_valid_vn_phone(&digits) {
                    Ok(SettingValue::Text(digits))
                } else {
                    Err(refuse(ErrorCode::InvalidPhone))
                }
            }
            SettingKind::Text(TextRule::Email) => {
                let email = raw.to_lowercase();
                validate_email(&email)?;
                if email.chars().count() > EMAIL_MAX {
                    return Err(refuse(ErrorCode::InvalidEmail));
                }
                Ok(SettingValue::Text(email))
            }
            SettingKind::Text(TextRule::HttpsUrl) => {
                let host = raw.strip_prefix(URL_SCHEME).unwrap_or_default();
                let valid = raw.chars().count() <= SETTING_VALUE_MAX
                    && !raw.chars().any(|c| c.is_whitespace() || c.is_control())
                    && host
                        .split(['/', '?', '#'])
                        .next()
                        .is_some_and(|h| h.contains('.') && !h.starts_with('.'));
                if valid {
                    Ok(SettingValue::Text(raw.to_owned()))
                } else {
                    Err(refuse(ErrorCode::InvalidUrl))
                }
            }
        }
    }
}

/// One setting as it stands: its value and, when it is off its default,
/// since when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingState {
    pub def: &'static SettingDef,
    pub value: SettingValue,
    /// `None`: at its default.
    pub changed_on: Option<DateTime<Utc>>,
}

/// Every setting as it stands. Read from storage on each use (no copy kept
/// in memory: the next request sees an admin's change).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    states: Vec<SettingState>,
}

impl Default for Settings {
    fn default() -> Self {
        Self::from_stored(&[])
    }
}

impl Settings {
    /// The registry over the stored rows: a row that no longer parses (a
    /// bound tightened since) or names no setting is skipped and logged,
    /// never fatal: the page still renders on the default.
    pub fn from_stored(stored: &[StoredSetting]) -> Self {
        for row in stored.iter().filter(|r| definition(&r.key).is_none()) {
            warn!(key = %row.key, "stored setting is not in the registry; ignored");
        }
        let states = SETTINGS
            .iter()
            .map(|def| {
                let row = stored.iter().find(|r| r.key == def.key);
                match row.map(|r| (r, def.parse(&r.value))) {
                    Some((r, Ok(value))) => SettingState {
                        def,
                        value,
                        changed_on: Some(r.updated_on),
                    },
                    Some((r, Err(_))) => {
                        warn!(key = %r.key, "stored setting is out of its bounds; default used");
                        SettingState {
                            def,
                            value: def.default_value(),
                            changed_on: None,
                        }
                    }
                    None => SettingState {
                        def,
                        value: def.default_value(),
                        changed_on: None,
                    },
                }
            })
            .collect();
        Self { states }
    }

    pub fn states(&self) -> &[SettingState] {
        &self.states
    }

    pub fn value(&self, key: &str) -> Option<&SettingValue> {
        self.states
            .iter()
            .find(|s| s.def.key == key)
            .map(|s| &s.value)
    }

    fn number(&self, key: &str) -> i64 {
        match self.value(key) {
            Some(SettingValue::Number(n)) => *n,
            _ => 0,
        }
    }

    fn switch(&self, key: &str) -> bool {
        matches!(self.value(key), Some(SettingValue::Switch(true)))
    }

    fn text(&self, key: &str) -> Option<&str> {
        match self.value(key) {
            Some(SettingValue::Text(t)) if !t.is_empty() => Some(t.as_str()),
            _ => None,
        }
    }

    // ── What the use cases and pages read ─────────────────────────────────────

    /// The platform's name an admin chose; `None`: the deployment's own.
    pub fn site_name(&self) -> Option<&str> {
        self.text(BRAND_NAME)
    }

    /// The platform's description an admin chose; `None`: the deployment's
    /// own, else the locale's.
    pub fn site_description(&self) -> Option<&str> {
        self.text(BRAND_DESCRIPTION)
    }

    /// An uploaded image setting's file.
    pub fn image(&self, key: &str) -> Option<Uuid> {
        self.text(key).and_then(|t| Uuid::parse_str(t).ok())
    }

    /// The platform's logo an admin uploaded; `None`: the built-in mark.
    pub fn logo(&self) -> Option<Uuid> {
        self.image(BRAND_LOGO)
    }

    /// The browser and home-screen icon: the one uploaded for it, else the
    /// logo; `None`: the built-in icons.
    pub fn icon(&self) -> Option<Uuid> {
        self.image(BRAND_ICON).or_else(|| self.logo())
    }

    pub fn support_phone(&self) -> Option<&str> {
        self.text(SUPPORT_PHONE)
    }

    pub fn support_zalo_url(&self) -> Option<&str> {
        self.text(SUPPORT_ZALO_URL)
    }

    pub fn support_email(&self) -> Option<&str> {
        self.text(SUPPORT_EMAIL)
    }

    /// Providers may sign themselves up.
    pub fn registration_open(&self) -> bool {
        self.switch(REGISTRATION_OPEN)
    }

    /// Who sees `feature`.
    pub fn stage(&self, feature: Feature) -> Stage {
        match self.value(feature.setting_key()) {
            Some(SettingValue::Stage(stage)) => *stage,
            _ => Stage::On,
        }
    }

    /// Whether `feature` is open to anyone at all (on, or in preview): a
    /// write of it is refused only while it is off, the gate keeping a
    /// preview to the accounts that may see it.
    pub fn is_open(&self, feature: Feature) -> bool {
        self.stage(feature) != Stage::Off
    }

    /// The features a viewer sees; `may_preview`: it holds
    /// `feature.preview`.
    pub fn features_shown(&self, may_preview: bool) -> FeaturesShown {
        FeaturesShown::new(|f| self.stage(f), may_preview)
    }

    /// Whether the admin put `feature`'s page at `place` (whether the
    /// viewer sees the feature is the caller's other question). A feature
    /// with no page is nowhere.
    pub fn placed(&self, feature: Feature, place: Place) -> bool {
        crate::features::placement_at(feature, place).is_some_and(|p| self.switch(p.key))
    }

    /// Job posts one client address may send a day.
    pub fn job_posts_per_client_per_day(&self) -> u32 {
        u32::try_from(self.number(JOB_POSTS_PER_CLIENT_PER_DAY)).unwrap_or(u32::MAX)
    }

    /// Open posts one phone may have within `job_post_days`.
    pub fn job_posts_open_per_phone(&self) -> u64 {
        u64::try_from(self.number(JOB_POSTS_OPEN_PER_PHONE)).unwrap_or_default()
    }

    /// Days a job post stays open to providers.
    pub fn job_post_days(&self) -> i64 {
        self.number(JOB_POST_DAYS)
    }

    /// How far from a job a provider is told about it, in metres.
    pub fn job_notify_radius_m(&self) -> f64 {
        (self.number(JOB_NOTIFY_RADIUS_KM) * METRES_PER_KM) as f64
    }

    /// At most this many providers (the nearest) are emailed about one job.
    pub fn job_notify_max(&self) -> i64 {
        self.number(JOB_NOTIFY_MAX)
    }

    /// Services one business may list (hidden ones count).
    pub fn services_per_business(&self) -> u64 {
        u64::try_from(self.number(SERVICES_PER_BUSINESS)).unwrap_or_default()
    }

    /// Supplies one business may list (hidden ones count).
    pub fn products_per_business(&self) -> u64 {
        u64::try_from(self.number(PRODUCTS_PER_BUSINESS)).unwrap_or_default()
    }

    /// A review written this soon after its contact request is flagged.
    pub fn review_suspect_minutes(&self) -> i64 {
        self.number(REVIEW_SUSPECT_MINUTES)
    }

    /// Leads a week an area needs every report week to be steady.
    pub fn steady_weekly_leads(&self) -> i64 {
        self.number(STEADY_WEEKLY_LEADS)
    }

    /// Sponsored shops moved up on a near-me page.
    pub fn sponsored_shown(&self) -> usize {
        usize::try_from(self.number(SPONSORED_SHOWN)).unwrap_or_default()
    }
}

/// Reads the settings for a use case or a page: one query over the small
/// `platform_settings` table each time, never a copy kept in memory.
#[derive(Clone)]
pub struct SettingsReader {
    repo: Arc<dyn SettingRepositoryTrait + Send + Sync>,
}

impl SettingsReader {
    pub fn new(repo: Arc<dyn SettingRepositoryTrait + Send + Sync>) -> Self {
        Self { repo }
    }

    pub async fn load(&self) -> Result<Settings> {
        Ok(Settings::from_stored(&self.repo.all().await?))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::Utc;

    use super::*;

    fn stored(key: &str, value: &str) -> StoredSetting {
        StoredSetting {
            key: key.to_owned(),
            value: value.to_owned(),
            updated_on: Utc::now(),
        }
    }

    #[test]
    fn keys_are_unique_dotted_and_fit_the_column() {
        let pattern_ok = |k: &str| {
            k.split('.').count() >= 2
                && k.split('.').all(|w| {
                    w.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                        && w.chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                })
        };
        let mut seen = HashSet::new();
        for d in SETTINGS {
            assert!(seen.insert(d.key), "duplicate setting {}", d.key);
            assert!(pattern_ok(d.key), "{} does not match KEY_PATTERN", d.key);
            assert!(d.key.len() <= 64, "{} is longer than the key column", d.key);
        }
    }

    #[test]
    fn every_default_is_within_its_bounds() {
        for d in SETTINGS {
            if let SettingKind::Number { default, min, max } = d.kind {
                assert!(
                    min <= default && default <= max,
                    "{} default out of bounds",
                    d.key
                );
                assert_eq!(d.parse(&default.to_string()).ok(), Some(d.default_value()));
            }
        }
    }

    #[test]
    fn with_nothing_stored_the_code_runs_on_its_old_constants() {
        let s = Settings::default();
        assert_eq!(s.steady_weekly_leads(), STEADY_WEEKLY_LEADS_DEFAULT);
        assert_eq!(s.sponsored_shown(), SPONSORED_SHOWN_DEFAULT);
        assert_eq!(s.services_per_business(), SERVICES_PER_BUSINESS_DEFAULT);
        assert_eq!(s.products_per_business(), PRODUCTS_PER_BUSINESS_DEFAULT);
        assert_eq!(s.job_post_days(), JOB_POST_DAYS_DEFAULT);
        assert_eq!(s.job_notify_radius_m(), 10_000.0);
        assert!(s.registration_open());
        for f in crate::features::Feature::ALL {
            assert_eq!(s.stage(f), Stage::On, "{}", f.code());
        }
        assert_eq!(s.support_phone(), None);
    }

    #[test]
    fn a_stored_value_wins_and_a_broken_or_unknown_one_is_ignored() {
        let s = Settings::from_stored(&[
            stored(STEADY_WEEKLY_LEADS, "8"),
            stored(SPONSORED_SHOWN, "99"),
            stored(REGISTRATION_OPEN, "false"),
            stored(SUPPORT_PHONE, "0912345678"),
            stored("old.setting", "1"),
        ]);
        assert_eq!(s.steady_weekly_leads(), 8);
        assert_eq!(s.sponsored_shown(), SPONSORED_SHOWN_DEFAULT);
        assert!(!s.registration_open());
        assert_eq!(s.support_phone(), Some("0912345678"));
    }

    #[test]
    fn values_are_checked_by_kind() {
        // A key missing from the registry fails every assertion below.
        let missing = SettingDef {
            key: "missing.key",
            group: SettingGroup::Support,
            kind: SettingKind::Switch { default: false },
        };
        let def = |k| definition(k).copied().unwrap_or(missing);
        let code = |r: Result<SettingValue>| r.err().map(|e| e.code());
        assert_eq!(
            code(def(STEADY_WEEKLY_LEADS).parse("abc")),
            Some(ErrorCode::InvalidNumber)
        );
        assert_eq!(
            code(def(STEADY_WEEKLY_LEADS).parse("0")),
            Some(ErrorCode::SettingOutOfRange)
        );
        assert_eq!(
            def(SUPPORT_PHONE).parse(" 0912 345 678 ").ok(),
            Some(SettingValue::Text("0912345678".into()))
        );
        assert_eq!(
            code(def(SUPPORT_PHONE).parse("12345")),
            Some(ErrorCode::InvalidPhone)
        );
        assert_eq!(
            def(SUPPORT_ZALO_URL)
                .parse("https://zalo.me/0912345678")
                .ok(),
            Some(SettingValue::Text("https://zalo.me/0912345678".into()))
        );
        assert_eq!(
            code(def(SUPPORT_ZALO_URL).parse("javascript:alert(1)")),
            Some(ErrorCode::InvalidUrl)
        );
        assert_eq!(
            code(def(SUPPORT_ZALO_URL).parse("http://zalo.me/1")),
            Some(ErrorCode::InvalidUrl)
        );
        assert_eq!(
            def(SUPPORT_EMAIL).parse("Ho.Tro@Agri.VN").ok(),
            Some(SettingValue::Text("ho.tro@agri.vn".into()))
        );
        assert_eq!(
            def(SUPPORT_EMAIL).parse("").ok(),
            Some(SettingValue::Text(String::new()))
        );
        assert_eq!(
            code(def(REGISTRATION_OPEN).parse("maybe")),
            Some(ErrorCode::InvalidRequestBody)
        );
        // The name lands in titles and email subjects: one line, short.
        assert_eq!(
            def(BRAND_NAME).parse(" Nông\r\nBcc: x@y.vn ").ok(),
            Some(SettingValue::Text("Nông Bcc: x@y.vn".into()))
        );
        assert_eq!(
            code(def(BRAND_NAME).parse(&"a".repeat(SITE_NAME_MAX + 1))),
            Some(ErrorCode::FieldTooLong)
        );
        assert_eq!(
            def(BRAND_NAME).parse(&"ă".repeat(SITE_NAME_MAX)).ok(),
            Some(SettingValue::Text("ă".repeat(SITE_NAME_MAX)))
        );
        assert_eq!(
            code(def(BRAND_DESCRIPTION).parse(&"a".repeat(SITE_DESCRIPTION_MAX + 1))),
            Some(ErrorCode::FieldTooLong)
        );
    }

    #[test]
    fn the_brand_falls_back_to_the_deployment_until_an_admin_names_it() {
        assert_eq!(Settings::default().site_name(), None);
        assert_eq!(Settings::default().site_description(), None);
        let s = Settings::from_stored(&[
            stored(BRAND_NAME, "Nông Kết Nối"),
            stored(BRAND_DESCRIPTION, "Thuê làm, mua vật tư."),
        ]);
        assert_eq!(s.site_name(), Some("Nông Kết Nối"));
        assert_eq!(s.site_description(), Some("Thuê làm, mua vật tư."));
        // A row stored past its bound (by hand, or before a bound tightened):
        // ignored.
        let broken = Settings::from_stored(&[stored(BRAND_NAME, &"x".repeat(SITE_NAME_MAX + 1))]);
        assert_eq!(broken.site_name(), None);
    }
}
