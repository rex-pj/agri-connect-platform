//! /admin/settings: the platform settings an admin changes without a deploy
//! (`setting.manage`), laid out like the other admin pages: one tab per
//! group (`application::settings::SettingGroup`, `?tab=`, each tab counting
//! the settings off their defaults), the group as one `admin-section` card
//! (title, status line, form, one actions row), and "back to defaults" as
//! its own card at the end, listing what it will change. Both are audited,
//! with an optional reason. A refused save re-shows the tab with the values
//! as typed and a message under each broken one; a saved one lands back on
//! its tab with a flash.

use std::collections::HashMap;

use application::{
    authz::Actor,
    constants::{ICON_MIN_SIDE, SITE_DESCRIPTION_ADVISED_MIN},
    features::{placement_at, placement_of, Feature, Place, Stage, PLACEMENTS},
    outputs::{SettingDto, SettingGroupDto},
    settings::{
        SettingGroup, SettingKind, BRAND_DESCRIPTION, BRAND_ICON, BRAND_LOGO, BRAND_NAME, SETTINGS,
    },
};
use axum::{extract::State, http::StatusCode, response::Response};
use chrono::{DateTime, Utc};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiMultipart, ApiQuery},
    features,
    locale::Locale,
    middleware::auth::Viewer,
    pages::{
        admin::{is_refusal, render_admin, url, AdminTab},
        forms::{reason, FormErrors},
        listing::with_query,
        see_other,
    },
    shell::{browse_pages, Link},
    views::SiteConfig,
};

/// The form's own fields; every other field is a setting's key.
const FIELD_GROUP: &str = "group";
const FIELD_REASON: &str = "reason";
const FIELD_ACTION: &str = "action";
/// `action` of the reset card's button; anything else saves.
const ACTION_RESET: &str = "reset";
/// The query parameter of the tab shown.
const PARAM_TAB: &str = "tab";
/// `SettingDto.kind` of an uploaded image (the logo, the browser icon).
const KIND_IMAGE: &str = "image";
/// `SettingDto.kind` of a feature's stage.
const KIND_STAGE: &str = "stage";
const MEBIBYTE: usize = 1024 * 1024;

/// The removal form's field: which image setting.
const FIELD_KEY: &str = "key";

/// An image setting as the identity tab shows it: a small tile, what it is
/// now, a file box named after the setting, and "use the default" behind a
/// confirmation while one is uploaded.
#[derive(Debug, Serialize)]
pub struct ImageView {
    pub key: &'static str,
    /// The key as an element id (`brand-logo`); the confirmation's is
    /// `remove-` and it.
    pub id: String,
    pub label: String,
    pub hint: String,
    /// The file uploaded; `None`: the default.
    pub current: Option<String>,
    /// `uploaded`, `logo` (the browser icon falling back on an uploaded
    /// logo) or `builtin`: the line under its label.
    pub state: &'static str,
    pub changed_on: Option<DateTime<Utc>>,
    /// A refused upload's message, under its box.
    pub error: Option<String>,
}

/// What the identity tab's preview falls back on while a box is empty.
#[derive(Debug, Serialize)]
pub struct BrandDefaults {
    /// `SITE_NAME`.
    pub name: String,
    /// `SITE_DESCRIPTION`, else the home page's own.
    pub description: String,
}

/// `?tab=`: the group shown; anything else is the first one.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SettingsQuery {
    pub tab: String,
}

/// One setting as its form shows it.
#[derive(Debug, Serialize)]
pub struct SettingView {
    pub key: &'static str,
    /// The key as an element id (`jobs-open_days`).
    pub id: String,
    /// `number`, `switch`, `phone`, `url`, `email` or `text`.
    pub kind: &'static str,
    pub label: String,
    pub hint: String,
    /// As typed when the form came back, else as stored.
    pub value: String,
    /// A switch box is ticked.
    pub on: bool,
    pub min: Option<i64>,
    pub max: Option<i64>,
    /// "Mặc định 20, từ 1 đến 1000."
    pub default_line: String,
    pub changed_on: Option<DateTime<Utc>>,
    pub error: Option<String>,
    /// A feature's stage (`kind` `stage`): its icon, where it shows, the
    /// three choices and, while in preview, the way to try its page.
    pub feature: Option<FeatureView>,
}

/// What a feature's row shows beside its three choices.
#[derive(Debug, Serialize)]
pub struct FeatureView {
    pub icon: &'static str,
    /// "Hiện ở: …": every way in that is hidden with it.
    pub where_line: String,
    pub stages: Vec<StageChoice>,
    /// Where its page is led to from (top bar, phone bar, footer); empty
    /// for a feature with no page of its own.
    pub places: Vec<PlaceChoice>,
    /// Its page, while it is in preview as stored: open it to try it.
    pub preview_href: Option<String>,
}

/// One place of a feature's page, a checkbox (`features::Placement`).
#[derive(Debug, Serialize)]
pub struct PlaceChoice {
    pub key: &'static str,
    pub id: String,
    /// `top`, `phone` or `footer`.
    pub place: &'static str,
    pub label: String,
    pub on: bool,
    pub error: Option<String>,
}

/// The features tab's preview of the frame: the wide bar, the phone's
/// and the footer as the boxes stand (Alpine `navPreview` keeps it in step
/// with what is ticked; without JS it shows the form as shown).
#[derive(Debug, Serialize)]
pub struct NavPreview {
    pub top: Vec<PreviewItem>,
    pub phone: Vec<PreviewItem>,
    pub footer: Vec<PreviewItem>,
    /// Each bar's room for features (`Place::room`), and how much is taken.
    pub top_max: usize,
    pub top_used: usize,
    pub phone_max: usize,
    pub phone_used: usize,
}

/// One link of the preview: a fixed one (home, services, the account), or
/// a feature's, shown while its place is ticked and it is not off.
#[derive(Debug, Serialize)]
pub struct PreviewItem {
    pub icon: &'static str,
    pub label: String,
    /// Its place's checkbox and its feature's stage radios; `None` fixed.
    pub place_key: Option<&'static str>,
    pub stage_key: Option<&'static str>,
    pub shown: bool,
    /// Its feature is in preview: only admins would see it.
    pub preview: bool,
}

/// One of a feature's three choices, a radio button.
#[derive(Debug, Serialize)]
pub struct StageChoice {
    pub value: &'static str,
    pub label: String,
    pub hint: String,
    pub checked: bool,
}

/// A line of the reset card: what goes back to what.
#[derive(Debug, Serialize)]
pub struct ResetLine {
    pub label: String,
    pub current: String,
    pub default: String,
}

/// The group shown.
#[derive(Debug, Serialize)]
pub struct GroupView {
    pub code: &'static str,
    pub icon: &'static str,
    pub title: String,
    pub sub: String,
    pub settings: Vec<SettingView>,
    /// Its uploaded images, each with its own form; empty: no image card.
    pub images: Vec<ImageView>,
    /// How many are off their defaults.
    pub changed: usize,
    /// The reset card's lines; empty: no reset card.
    pub resets: Vec<ResetLine>,
    /// The features tab's preview of the frame.
    pub nav: Option<NavPreview>,
}

#[derive(Debug, Serialize)]
pub struct SettingsPage {
    pub tabs: Vec<AdminTab>,
    pub group: GroupView,
    pub action: String,
    /// Where the identity tab saves (multipart) and removes an image.
    pub brand_action: String,
    pub image_remove_action: String,
    /// What its preview falls back on.
    pub defaults: BrandDefaults,
    /// The introduction's advised shortest length, for its counter.
    pub description_advised: usize,
    /// The largest upload, in MB, for the photo box's line.
    pub max_mb: usize,
    pub flash_text: Option<String>,
    /// The message of a refused save, above the card.
    pub error: Option<String>,
}

const fn group_icon(group: SettingGroup) -> &'static str {
    match group {
        SettingGroup::Brand => "fa-id-badge",
        SettingGroup::Support => "fa-headset",
        SettingGroup::Features => "fa-toggle-on",
        SettingGroup::Jobs => "fa-bullhorn",
        SettingGroup::Listings => "fa-store",
        SettingGroup::Market => "fa-chart-column",
    }
}

fn switch_word(locale: &Locale, value: &str) -> String {
    locale.text(if value == "true" {
        "settings.on"
    } else {
        "settings.off"
    })
}

/// A stage (`on`, `preview`, `off`) as the page words it.
fn stage_word(locale: &Locale, value: &str) -> String {
    locale.text(&format!("settings.stage.{value}"))
}

/// A feature's row on the features tab: `stored` its stage as saved,
/// `value` as shown (typed, when the form came back).
/// `shown` gives each box's value as the form shows it, `error_of` its
/// message.
fn feature_view(
    locale: &Locale,
    key: &str,
    stored: &str,
    value: &str,
    shown: &impl Fn(&str) -> String,
    error_of: &impl Fn(&str) -> Option<String>,
) -> Option<FeatureView> {
    let feature = Feature::of_key(key)?;
    let places = Place::ALL
        .iter()
        .filter_map(|p| placement_at(feature, *p))
        .map(|p| PlaceChoice {
            key: p.key,
            id: p.key.replace('.', "-"),
            place: p.place.code(),
            label: locale.text(&format!("settings.place.{}", p.place.code())),
            on: shown(p.key) == "true",
            error: error_of(p.key),
        })
        .collect();
    let preview_href = (stored == Stage::Preview.code())
        .then(|| features::page_route(feature))
        .flatten()
        .and_then(|route| url(route, &[]).ok());
    Some(FeatureView {
        icon: feature.icon(),
        where_line: locale.text(&format!("settings.key.{key}_where")),
        stages: Stage::ALL
            .iter()
            .map(|s| StageChoice {
                value: s.code(),
                label: stage_word(locale, s.code()),
                hint: locale.text(&format!("settings.stage_hint.{}", s.code())),
                checked: s.code() == value,
            })
            .collect(),
        places,
        preview_href,
    })
}

/// The frame as the features tab's boxes stand (`shown`): the fixed pages
/// always, a feature's while its place is ticked and it is not off.
fn nav_preview(locale: &Locale, shown: &impl Fn(&str) -> String) -> NavPreview {
    let (pages, account) = browse_pages(locale);
    let item = |feature: Option<Feature>, link: &Link, place: Place| {
        let place_key = feature.and_then(|f| placement_at(f, place)).map(|p| p.key);
        let stage_key = feature.map(Feature::setting_key);
        let stage = stage_key.map(shown);
        PreviewItem {
            icon: link.icon,
            label: link.label.clone(),
            shown: place_key.is_none_or(|k| shown(k) == "true")
                && stage.as_deref() != Some(Stage::Off.code()),
            preview: stage.as_deref() == Some(Stage::Preview.code()),
            place_key,
            stage_key,
        }
    };
    // No more features on a bar than its room, the first kept, as the
    // frame draws it (`shell::placed_pages`).
    let bar = |place: Place| -> Vec<PreviewItem> {
        let mut taken = 0;
        pages
            .iter()
            .map(|(f, link)| {
                let mut i = item(*f, link, place);
                if i.place_key.is_some() && i.shown {
                    i.shown = place.room().is_none_or(|room| taken < room);
                    taken += usize::from(i.shown);
                }
                i
            })
            .collect()
    };
    let used = |place: Place| {
        PLACEMENTS
            .iter()
            .filter(|p| p.place == place && shown(p.key) == "true")
            // An off feature's tick takes no room (`SettingsUseCase`).
            .filter(|p| shown(p.feature.setting_key()) != Stage::Off.code())
            .count()
    };
    let mut phone = bar(Place::Phone);
    phone.push(item(None, &account, Place::Phone));
    NavPreview {
        top: bar(Place::Top),
        phone,
        footer: bar(Place::Footer),
        top_max: Place::Top.room().unwrap_or_default(),
        top_used: used(Place::Top),
        phone_max: Place::Phone.room().unwrap_or_default(),
        phone_used: used(Place::Phone),
    }
}

/// A value as the page words it: a switch on / off, an empty text "empty".
fn shown_value(locale: &Locale, kind: &str, value: &str) -> String {
    match (kind, value) {
        ("switch", v) => switch_word(locale, v),
        (KIND_STAGE, v) => stage_word(locale, v),
        (_, "") => locale.text("settings.empty"),
        (_, v) => v.to_owned(),
    }
}

/// What the pages show while a `brand.*` setting is empty: the
/// deployment's own name and description. `None` for any other key (empty:
/// not shown).
fn deployment_default(locale: &Locale, site: &SiteConfig, key: &str) -> Option<String> {
    match key {
        BRAND_NAME => Some(site.name.clone()),
        BRAND_DESCRIPTION => Some(
            site.description
                .clone()
                .unwrap_or_else(|| locale.text("site.description")),
        ),
        _ => None,
    }
}

/// The words of a setting's default and bounds.
fn default_line(locale: &Locale, site: &SiteConfig, s: &SettingDto) -> String {
    match (s.kind, s.min, s.max) {
        ("number", Some(min), Some(max)) => locale.text_with(
            "settings.default_number",
            &[("value", &s.default), ("min", &min), ("max", &max)],
        ),
        ("switch", _, _) => locale.text_with(
            "settings.default_switch",
            &[("value", &switch_word(locale, &s.default))],
        ),
        (KIND_STAGE, _, _) => locale.text_with(
            "settings.default_stage",
            &[("value", &stage_word(locale, &s.default))],
        ),
        _ => match deployment_default(locale, site, s.key) {
            Some(value) => locale.text_with("settings.default_deployment", &[("value", &value)]),
            None => locale.text("settings.default_empty"),
        },
    }
}

fn label_of(locale: &Locale, key: &str) -> String {
    // A feature's place: "Giá nông sản · Chân trang".
    if let Some(p) = placement_of(key) {
        return format!(
            "{} · {}",
            label_of(locale, p.feature.setting_key()),
            locale.text(&format!("settings.place.{}", p.place.code()))
        );
    }
    locale.text(&format!("settings.key.{key}"))
}

/// The tab of `group` from the stored settings; `typed` (a refused save's
/// values and messages) shows its form as it was sent, a refused image's
/// message under its box.
fn group_view(
    locale: &Locale,
    site: &SiteConfig,
    group: SettingGroup,
    stored: &SettingGroupDto,
    typed: Option<(&HashMap<String, String>, &FormErrors)>,
) -> GroupView {
    let logo_uploaded = stored
        .settings
        .iter()
        .any(|s| s.key == BRAND_LOGO && !s.value.is_empty());
    let images = stored
        .settings
        .iter()
        .filter(|s| s.kind == KIND_IMAGE)
        .map(|s| ImageView {
            key: s.key,
            id: s.key.replace('.', "-"),
            label: label_of(locale, s.key),
            hint: locale.text_with(
                &format!("settings.key.{}_hint", s.key),
                &[("min", &ICON_MIN_SIDE)],
            ),
            current: Some(s.value.clone()).filter(|v| !v.is_empty()),
            state: match (s.value.is_empty(), s.key) {
                (false, _) => "uploaded",
                (true, BRAND_ICON) if logo_uploaded => "logo",
                (true, _) => "builtin",
            },
            changed_on: s.changed_on,
            error: typed.and_then(|(_, errors)| errors.fields.get(s.key).cloned()),
        })
        .collect();
    // A box's value as the form shows it: as typed; a box not sent keeps
    // its stored value, except a switch, which the browser leaves out when
    // it is off.
    let shown = |key: &str| -> String {
        let s = stored.settings.iter().find(|s| s.key == key);
        match typed.map(|(values, _)| values.get(key)) {
            Some(Some(v)) => v.clone(),
            Some(None) if s.is_some_and(|s| s.kind == "switch") => String::new(),
            _ => s.map(|s| s.value.clone()).unwrap_or_default(),
        }
    };
    let error_of = |key: &str| typed.and_then(|(_, errors)| errors.fields.get(key).cloned());
    let settings = stored
        .settings
        .iter()
        // A feature's places are drawn in its own row.
        .filter(|s| s.kind != KIND_IMAGE && placement_of(s.key).is_none())
        .map(|s| {
            let value = shown(s.key);
            let feature = feature_view(locale, s.key, &s.value, &value, &shown, &error_of);
            SettingView {
                key: s.key,
                id: s.key.replace('.', "-"),
                kind: s.kind,
                label: label_of(locale, s.key),
                hint: locale.text(&format!("settings.key.{}_hint", s.key)),
                on: value == "true",
                value,
                min: s.min,
                max: s.max,
                default_line: default_line(locale, site, s),
                changed_on: s.changed_on,
                error: error_of(s.key),
                feature,
            }
        })
        .collect();
    // Back to defaults never deletes an upload (`SettingsUseCase::reset`).
    let resets = stored
        .settings
        .iter()
        .filter(|s| s.changed_on.is_some() && s.kind != KIND_IMAGE)
        .map(|s| ResetLine {
            label: label_of(locale, s.key),
            current: shown_value(locale, s.kind, &s.value),
            default: deployment_default(locale, site, s.key)
                .unwrap_or_else(|| shown_value(locale, s.kind, &s.default)),
        })
        .collect();
    GroupView {
        code: group.code(),
        icon: group_icon(group),
        title: locale.text(&format!("settings.group.{}", group.code())),
        sub: locale.text(&format!("settings.group.{}_sub", group.code())),
        settings,
        images,
        changed: stored.changed,
        resets,
        nav: (group == SettingGroup::Features).then(|| nav_preview(locale, &shown)),
    }
}

/// The address of `group`'s tab.
fn tab_href(group: SettingGroup) -> Result<String, Error> {
    Ok(with_query(
        &url("admin_settings", &[])?,
        &[(PARAM_TAB, group.code())],
    ))
}

/// One tab per group, each counting its settings off their defaults.
fn tabs(
    locale: &Locale,
    stored: &[SettingGroupDto],
    current: SettingGroup,
) -> Result<Vec<AdminTab>, Error> {
    SettingGroup::ALL
        .into_iter()
        .map(|g| {
            let changed = stored
                .iter()
                .find(|s| s.code == g.code())
                .map_or(0, |s| s.changed);
            Ok(AdminTab {
                label: locale.text(&format!("settings.group.{}", g.code())),
                href: tab_href(g)?,
                count: (changed > 0).then_some(changed),
                current: g == current,
            })
        })
        .collect()
}

struct View<'a> {
    group: SettingGroup,
    /// A refused save: the form as sent and what was wrong with it.
    typed: Option<(&'a HashMap<String, String>, &'a FormErrors)>,
    flash: Option<Flash>,
    status: StatusCode,
}

async fn page(state: &AppState, actor: &Actor, view: View<'_>) -> Result<Response, Error> {
    let locale = state.locale();
    let stored = state.usecases.settings.groups(actor).await?;
    let current = stored
        .iter()
        .find(|g| g.code == view.group.code())
        .ok_or_else(|| Error::internal("settings group missing from the page"))?;
    let action = url("admin_settings", &[])?;
    let site = state.views.site();
    let body = SettingsPage {
        tabs: tabs(locale, &stored, view.group)?,
        group: group_view(locale, site, view.group, current, view.typed),
        action: action.clone(),
        brand_action: url("admin_settings_brand", &[])?,
        image_remove_action: url("admin_settings_image_delete", &[])?,
        description_advised: SITE_DESCRIPTION_ADVISED_MIN,
        max_mb: state.usecases.media.max_upload_bytes().div_ceil(MEBIBYTE),
        defaults: BrandDefaults {
            name: site.name.clone(),
            description: site
                .description
                .clone()
                .unwrap_or_else(|| locale.text("home.meta")),
        },
        flash_text: view.flash.and_then(|f| match f {
            Flash::SettingsSaved => Some(locale.text("admin.saved")),
            Flash::SettingsReset => Some(locale.text("settings.reset_done")),
            Flash::ImageRemoved => Some(locale.text("settings.image_removed")),
            _ => None,
        }),
        error: view.typed.and_then(|(_, errors)| errors.message.clone()),
    };
    render_admin(
        state,
        actor,
        &action,
        view.status,
        "pages/admin/settings.html",
        &body,
    )
}

/// GET /admin/settings
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<SettingsQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let view = View {
        group: SettingGroup::parse(&q.tab).unwrap_or(SettingGroup::ALL[0]),
        typed: None,
        flash: flash.0,
        status: StatusCode::OK,
    };
    let mut response = page(&state, &actor, view).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// The values a group's form sent, every key of the group: a switch not
/// ticked is not sent by the browser, so it is off.
fn group_values(group: SettingGroup, form: &HashMap<String, String>) -> Vec<(String, String)> {
    SETTINGS
        .iter()
        .filter(|d| d.group == group)
        .filter_map(|d| match (form.get(d.key), d.kind) {
            (Some(v), _) => Some((d.key.to_owned(), v.clone())),
            (None, SettingKind::Switch { .. }) => Some((d.key.to_owned(), "false".to_owned())),
            (None, _) => None,
        })
        .collect()
}

fn reason_of(form: &HashMap<String, String>) -> Option<String> {
    reason(
        form.get(FIELD_REASON)
            .map(String::as_str)
            .unwrap_or_default(),
    )
}

/// POST /admin/settings: save a group's values, or put them back on their
/// defaults (`action=reset`).
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<HashMap<String, String>>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let code = form.get(FIELD_GROUP).cloned().unwrap_or_default();
    let group = SettingGroup::parse(&code)
        .ok_or_else(|| Error::new(ErrorCode::UnknownSetting).with_param("key", &code))?;
    let resetting = form.get(FIELD_ACTION).is_some_and(|a| a == ACTION_RESET);
    let settings = &state.usecases.settings;
    let result = if resetting {
        settings.reset(&actor, group.code(), reason_of(&form)).await
    } else {
        // Only this group's keys: a stray field never reaches the use case.
        settings
            .update(&actor, &group_values(group, &form), reason_of(&form))
            .await
    };
    let flash = if resetting {
        Flash::SettingsReset
    } else {
        Flash::SettingsSaved
    };
    finish(&state, &actor, group, &form, result, flash).await
}

/// POST /admin/settings/brand (multipart): the identity tab's one save, its
/// words and the logo and browser icon chosen (`brand.logo`, `brand.icon`:
/// the file boxes are named after their settings). Who is asking is
/// checked before the body is read.
pub async fn save_brand(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    upload: ApiMultipart,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let settings = &state.usecases.settings;
    settings.may_manage(&actor)?;
    let max = state.usecases.media.max_upload_bytes();
    let (form, files): (HashMap<String, String>, _) = upload
        .into_form_files(&[BRAND_LOGO, BRAND_ICON], max)
        .await?;
    let group = SettingGroup::Brand;
    let result = settings
        .save_brand(&actor, &group_values(group, &form), files, reason_of(&form))
        .await;
    finish(&state, &actor, group, &form, result, Flash::SettingsSaved).await
}

/// POST /admin/settings/image/delete (`key`, `reason`): back on the built-in
/// logo or icon, behind its own confirmation.
pub async fn remove_image(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<HashMap<String, String>>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let key = form.get(FIELD_KEY).cloned().unwrap_or_default();
    let result = state
        .usecases
        .settings
        .remove_image(&actor, &key, reason_of(&form))
        .await;
    // The tab as stored, not the removal form's two fields as typed boxes.
    let shown = HashMap::new();
    finish(
        &state,
        &actor,
        SettingGroup::Brand,
        &shown,
        result,
        Flash::ImageRemoved,
    )
    .await
}

/// Back on `group`'s tab with `flash`, or the tab again with the form as
/// sent and a message under each broken box.
async fn finish(
    state: &AppState,
    actor: &Actor,
    group: SettingGroup,
    form: &HashMap<String, String>,
    result: Result<(), Error>,
    flash: Flash,
) -> Result<Response, Error> {
    match result {
        Ok(()) => {
            let mut response = see_other(&tab_href(group)?)?;
            flash.set_on(&mut response);
            Ok(response)
        }
        Err(err) if is_refusal(&err) => {
            let errors = FormErrors::from_error(state, &err);
            let view = View {
                group,
                typed: Some((form, &errors)),
                flash: None,
                status: err.kind().status(),
            };
            page(state, actor, view).await
        }
        Err(err) => Err(err),
    }
}
