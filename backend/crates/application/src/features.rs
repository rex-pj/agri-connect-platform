//! The parts of the platform an admin opens to farmers when they are ready
//! (`/admin/settings?tab=features`): each one a setting of kind `Stage`.
//!
//! A feature is on (everyone), in preview (only an account with
//! `feature.preview` sees it, to try it on the real site first) or off
//! (nobody: its pages answer like a page that does not exist). What it
//! covers is decided in one place each: its addresses by the web layer's
//! gate (`web::features`), its ways in by the frame and the pages that
//! read `FeaturesShown`, its writes by the use cases that check
//! `Settings::stage`. The admin's own pages of a feature stay open, so its
//! data can be prepared before it is opened. Turning one off never deletes
//! anything.
//!
//! Only what is a whole errand for a farmer has a switch. What shows only
//! when it has data (alerts, sponsored shops, partners) needs none: with
//! nothing entered it is not shown.

use serde::{ser::SerializeMap, Serialize, Serializer};

/// Who sees a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    On,
    /// Only accounts with `feature.preview` (the admins).
    Preview,
    Off,
}

/// The stored text of a stage.
const ON: &str = "on";
const PREVIEW: &str = "preview";
const OFF: &str = "off";
/// A switch's stored text, read as a stage: the job-post feature was a
/// switch (`access.job_posts_open`) before it was a stage.
const LEGACY_ON: &str = "true";
const LEGACY_OFF: &str = "false";

impl Stage {
    pub const ALL: [Stage; 3] = [Stage::On, Stage::Preview, Stage::Off];

    /// As stored and as the form sends it; also its locale key
    /// (`settings.stage.<code>`).
    pub const fn code(self) -> &'static str {
        match self {
            Stage::On => ON,
            Stage::Preview => PREVIEW,
            Stage::Off => OFF,
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            ON | LEGACY_ON => Some(Stage::On),
            PREVIEW => Some(Stage::Preview),
            OFF | LEGACY_OFF => Some(Stage::Off),
            _ => None,
        }
    }

    /// Whether a viewer sees it; `may_preview`: the viewer holds
    /// `feature.preview`.
    pub const fn shown_to(self, may_preview: bool) -> bool {
        match self {
            Stage::On => true,
            Stage::Preview => may_preview,
            Stage::Off => false,
        }
    }
}

/// Every part of the platform with a switch, in the settings tab's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// The farm-gate price board (`/prices`, the home page's price strip).
    Prices,
    /// Services and shops near a position (`/explore`).
    NearMe,
    /// Farm supplies: their lists and pages, a provider's own.
    Supplies,
    /// The provider directory (`/providers`); a provider's own page stays.
    Directory,
    /// Reviews: writing one, the stars, a provider's answers.
    Reviews,
    /// Hire requests: a farmer's post, a provider's list of them.
    Jobs,
}

impl Feature {
    pub const ALL: [Feature; 6] = [
        Feature::Prices,
        Feature::NearMe,
        Feature::Supplies,
        Feature::Directory,
        Feature::Reviews,
        Feature::Jobs,
    ];

    /// Its stable name: the key of `FeaturesShown` (templates read
    /// `shell.features.<code>`), the JSON API's and the locale's
    /// (`features.<code>`).
    pub const fn code(self) -> &'static str {
        match self {
            Feature::Prices => "prices",
            Feature::NearMe => "near_me",
            Feature::Supplies => "supplies",
            Feature::Directory => "directory",
            Feature::Reviews => "reviews",
            Feature::Jobs => "jobs",
        }
    }

    /// The setting holding its stage. The job posts keep the key they had
    /// as a switch, so a stored "off" stays off.
    pub const fn setting_key(self) -> &'static str {
        match self {
            Feature::Prices => "features.prices",
            Feature::NearMe => "features.near_me",
            Feature::Supplies => "features.supplies",
            Feature::Directory => "features.directory",
            Feature::Reviews => "features.reviews",
            Feature::Jobs => "access.job_posts_open",
        }
    }

    /// The feature a setting key holds the stage of.
    pub fn of_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.setting_key() == key)
    }

    /// Its icon on the settings tab, the same as its tab in the frame.
    pub const fn icon(self) -> &'static str {
        match self {
            Feature::Prices => "fa-chart-line",
            Feature::NearMe => "fa-location-dot",
            Feature::Supplies => "fa-bag-shopping",
            Feature::Directory => "fa-store",
            Feature::Reviews => "fa-star",
            Feature::Jobs => "fa-bullhorn",
        }
    }

    /// Whether it has a page of its own to lead to from the menus (the
    /// reviews live on the provider pages: no menu entry).
    pub const fn has_page(self) -> bool {
        !matches!(self, Feature::Reviews)
    }
}

/// Where a feature's page may be led to from the frame, each place a switch
/// on the features tab (`Placement`). The home page, the services and the
/// account are always there; a feature hidden from the viewer is never
/// shown, whatever its places.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// The top bar of a wide screen (1200 px and more): room for
    /// `TOP_TABS_MAX` features.
    Top,
    /// The tab bar of a phone: room for `PHONE_TABS_MAX` features.
    Phone,
    /// The footer's browse links.
    Footer,
}

/// The features a phone's tab bar has room for, beside home, the services
/// and the account (five tabs in all).
pub const PHONE_TABS_MAX: usize = 2;
/// The features the wide top bar has room for, beside home and the services,
/// between the brand and the search, language and account buttons of a
/// 1280 px screen (six tabs in all).
pub const TOP_TABS_MAX: usize = 4;

impl Place {
    pub const ALL: [Place; 3] = [Place::Top, Place::Phone, Place::Footer];

    /// Its locale key (`settings.place.<code>`) and form id.
    pub const fn code(self) -> &'static str {
        match self {
            Place::Top => "top",
            Place::Phone => "phone",
            Place::Footer => "footer",
        }
    }

    /// How many features it has room for; `None`: as many as there are.
    pub const fn room(self) -> Option<usize> {
        match self {
            Place::Top => Some(TOP_TABS_MAX),
            Place::Phone => Some(PHONE_TABS_MAX),
            Place::Footer => None,
        }
    }
}

/// One feature's place: its setting and whether it is there by default (the
/// frame as it was before it could be chosen: near me and supplies
/// everywhere, the directory and the prices on the wide bar and in the
/// footer, hire requests nowhere).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub feature: Feature,
    pub place: Place,
    pub key: &'static str,
    pub default: bool,
}

const fn placement(feature: Feature, place: Place, key: &'static str, default: bool) -> Placement {
    Placement {
        feature,
        place,
        key,
        default,
    }
}

/// Every feature's places, in the features tab's order.
pub const PLACEMENTS: [Placement; 15] = [
    placement(Feature::Prices, Place::Top, "nav.prices.top", true),
    placement(Feature::Prices, Place::Phone, "nav.prices.phone", false),
    placement(Feature::Prices, Place::Footer, "nav.prices.footer", true),
    placement(Feature::NearMe, Place::Top, "nav.near_me.top", true),
    placement(Feature::NearMe, Place::Phone, "nav.near_me.phone", true),
    placement(Feature::NearMe, Place::Footer, "nav.near_me.footer", true),
    placement(Feature::Supplies, Place::Top, "nav.supplies.top", true),
    placement(Feature::Supplies, Place::Phone, "nav.supplies.phone", true),
    placement(
        Feature::Supplies,
        Place::Footer,
        "nav.supplies.footer",
        true,
    ),
    placement(Feature::Directory, Place::Top, "nav.directory.top", true),
    placement(
        Feature::Directory,
        Place::Phone,
        "nav.directory.phone",
        false,
    ),
    placement(
        Feature::Directory,
        Place::Footer,
        "nav.directory.footer",
        true,
    ),
    placement(Feature::Jobs, Place::Top, "nav.jobs.top", false),
    placement(Feature::Jobs, Place::Phone, "nav.jobs.phone", false),
    placement(Feature::Jobs, Place::Footer, "nav.jobs.footer", false),
];

/// The placement a setting key holds.
pub fn placement_of(key: &str) -> Option<&'static Placement> {
    PLACEMENTS.iter().find(|p| p.key == key)
}

/// The placement of `feature` at `place`; `None` for a feature with no page.
pub fn placement_at(feature: Feature, place: Place) -> Option<&'static Placement> {
    PLACEMENTS
        .iter()
        .find(|p| p.feature == feature && p.place == place)
}

/// Whether `feature`'s page is at `place` by default (no setting stored).
pub fn default_placed(feature: Feature, place: Place) -> bool {
    placement_at(feature, place).is_some_and(|p| p.default)
}

/// Which features one viewer sees: what the frame, the pages and the JSON
/// API read. Serialized as `{ "prices": true, … }` by code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeaturesShown([bool; Feature::ALL.len()]);

impl FeaturesShown {
    /// Every feature shown: a frame built before the settings are read.
    pub const ALL: Self = Self([true; Feature::ALL.len()]);

    /// From each feature's stage (`stage`), for a viewer who may preview or
    /// not.
    pub fn new(stage: impl Fn(Feature) -> Stage, may_preview: bool) -> Self {
        Self(Feature::ALL.map(|f| stage(f).shown_to(may_preview)))
    }

    pub fn has(&self, feature: Feature) -> bool {
        Feature::ALL
            .iter()
            .position(|f| *f == feature)
            .and_then(|i| self.0.get(i).copied())
            .unwrap_or(false)
    }
}

impl Default for FeaturesShown {
    fn default() -> Self {
        Self::ALL
    }
}

impl Serialize for FeaturesShown {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(Feature::ALL.len()))?;
        for (feature, shown) in Feature::ALL.iter().zip(self.0) {
            map.serialize_entry(feature.code(), &shown)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn codes_and_keys_are_unique() {
        let codes: HashSet<_> = Feature::ALL.iter().map(|f| f.code()).collect();
        let keys: HashSet<_> = Feature::ALL.iter().map(|f| f.setting_key()).collect();
        assert_eq!(codes.len(), Feature::ALL.len());
        assert_eq!(keys.len(), Feature::ALL.len());
        for f in Feature::ALL {
            assert_eq!(Feature::of_key(f.setting_key()), Some(f));
        }
    }

    #[test]
    fn a_stage_reads_its_own_text_and_a_former_switch() {
        for s in Stage::ALL {
            assert_eq!(Stage::parse(s.code()), Some(s));
        }
        assert_eq!(Stage::parse("true"), Some(Stage::On));
        assert_eq!(Stage::parse("false"), Some(Stage::Off));
        assert_eq!(Stage::parse("maybe"), None);
    }

    #[test]
    fn preview_is_shown_only_to_who_may_preview() {
        assert!(Stage::On.shown_to(false));
        assert!(!Stage::Preview.shown_to(false));
        assert!(Stage::Preview.shown_to(true));
        assert!(!Stage::Off.shown_to(true));
    }

    #[test]
    fn every_feature_with_a_page_has_each_place_once_and_the_phone_fits() {
        let mut keys = HashSet::new();
        for f in Feature::ALL {
            for p in Place::ALL {
                let found = PLACEMENTS
                    .iter()
                    .filter(|x| x.feature == f && x.place == p)
                    .count();
                assert_eq!(
                    found,
                    usize::from(f.has_page()),
                    "{} {}",
                    f.code(),
                    p.code()
                );
            }
        }
        for p in PLACEMENTS {
            assert!(keys.insert(p.key), "{} twice", p.key);
            assert_eq!(placement_of(p.key), Some(&p));
        }
        for place in Place::ALL {
            let by_default = PLACEMENTS
                .iter()
                .filter(|p| p.place == place && p.default)
                .count();
            assert!(
                place.room().is_none_or(|room| by_default <= room),
                "{}",
                place.code()
            );
        }
    }

    #[test]
    fn the_shown_set_serializes_by_code() {
        let shown = FeaturesShown::new(
            |f| {
                if f == Feature::Prices {
                    Stage::Off
                } else {
                    Stage::On
                }
            },
            false,
        );
        assert!(!shown.has(Feature::Prices) && shown.has(Feature::Jobs));
        let json = serde_json::to_value(shown).unwrap_or_default();
        assert_eq!(json.get("prices"), Some(&serde_json::Value::Bool(false)));
        assert_eq!(json.get("near_me"), Some(&serde_json::Value::Bool(true)));
    }
}
