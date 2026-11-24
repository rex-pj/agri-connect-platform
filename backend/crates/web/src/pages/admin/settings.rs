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
    constants::ICON_MIN_SIDE,
    outputs::{SettingDto, SettingGroupDto},
    settings::{SettingGroup, SettingKind, BRAND_DESCRIPTION, BRAND_NAME, SETTINGS},
};
use axum::{extract::State, http::StatusCode, response::Response};
use chrono::{DateTime, Utc};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::IMAGE_FIELD,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiMultipart, ApiQuery},
    locale::Locale,
    middleware::auth::Viewer,
    pages::{
        admin::{is_refusal, render_admin, url, AdminTab},
        forms::{reason, FormErrors},
        listing::with_query,
        see_other,
    },
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
const MEBIBYTE: usize = 1024 * 1024;

/// The upload and removal forms of an image setting: which one.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ImageForm {
    pub key: String,
    pub reason: String,
}

/// An image setting as its card shows it: the upload box, and "back to the
/// built-in one" behind a confirmation while one is uploaded.
#[derive(Debug, Serialize)]
pub struct ImageView {
    pub key: &'static str,
    /// The key as an element id (`brand-logo`); the confirmation's is
    /// `remove-` and it.
    pub id: String,
    pub label: String,
    pub hint: String,
    /// The file uploaded; `None`: the built-in one.
    pub current: Option<String>,
    pub changed_on: Option<DateTime<Utc>>,
    /// A refused upload's message, under its box.
    pub error: Option<String>,
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
}

#[derive(Debug, Serialize)]
pub struct SettingsPage {
    pub tabs: Vec<AdminTab>,
    pub group: GroupView,
    pub action: String,
    /// Where an image card uploads (multipart) and removes.
    pub image_action: String,
    pub image_remove_action: String,
    /// The smallest logo side, for the card's line.
    pub icon_min: u32,
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
        SettingGroup::Access => "fa-door-open",
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

/// A value as the page words it: a switch on / off, an image uploaded or
/// the built-in one, an empty text "empty".
fn shown_value(locale: &Locale, kind: &str, value: &str) -> String {
    match (kind, value) {
        ("switch", v) => switch_word(locale, v),
        (KIND_IMAGE, "") => locale.text("settings.image_builtin"),
        (KIND_IMAGE, _) => locale.text("settings.image_uploaded"),
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
        _ => match deployment_default(locale, site, s.key) {
            Some(value) => locale.text_with("settings.default_deployment", &[("value", &value)]),
            None => locale.text("settings.default_empty"),
        },
    }
}

fn label_of(locale: &Locale, key: &str) -> String {
    locale.text(&format!("settings.key.{key}"))
}

/// The tab of `group` from the stored settings; `typed` (a refused save's
/// values and messages) shows its form as it was sent, `image_error` a
/// refused upload's message under its box.
fn group_view(
    locale: &Locale,
    site: &SiteConfig,
    group: SettingGroup,
    stored: &SettingGroupDto,
    typed: Option<(&HashMap<String, String>, &FormErrors)>,
    image_error: Option<&(String, String)>,
) -> GroupView {
    let images = stored
        .settings
        .iter()
        .filter(|s| s.kind == KIND_IMAGE)
        .map(|s| ImageView {
            key: s.key,
            id: s.key.replace('.', "-"),
            label: label_of(locale, s.key),
            hint: locale.text(&format!("settings.key.{}_hint", s.key)),
            current: Some(s.value.clone()).filter(|v| !v.is_empty()),
            changed_on: s.changed_on,
            error: image_error
                .filter(|(key, _)| key == s.key)
                .map(|(_, message)| message.clone()),
        })
        .collect();
    let settings = stored
        .settings
        .iter()
        .filter(|s| s.kind != KIND_IMAGE)
        .map(|s| {
            // As typed; a box not sent keeps its stored value, except a
            // switch, which the browser leaves out when it is off.
            let typed_value = typed.and_then(|(values, _)| match values.get(s.key) {
                Some(v) => Some(v.clone()),
                None if s.kind == "switch" => Some(String::new()),
                None => None,
            });
            let value = typed_value.unwrap_or_else(|| s.value.clone());
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
                error: typed.and_then(|(_, errors)| errors.fields.get(s.key).cloned()),
            }
        })
        .collect();
    let resets = stored
        .settings
        .iter()
        .filter(|s| s.changed_on.is_some())
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
    typed: Option<(&'a HashMap<String, String>, &'a FormErrors)>,
    /// A refused upload: its setting's key and the message.
    image_error: Option<(String, String)>,
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
    let body = SettingsPage {
        tabs: tabs(locale, &stored, view.group)?,
        group: group_view(
            locale,
            state.views.site(),
            view.group,
            current,
            view.typed,
            view.image_error.as_ref(),
        ),
        action: action.clone(),
        image_action: url("admin_settings_image", &[])?,
        image_remove_action: url("admin_settings_image_delete", &[])?,
        icon_min: ICON_MIN_SIDE,
        max_mb: state.usecases.media.max_upload_bytes().div_ceil(MEBIBYTE),
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
        image_error: None,
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
    let why = reason(
        form.get(FIELD_REASON)
            .map(String::as_str)
            .unwrap_or_default(),
    );
    let resetting = form.get(FIELD_ACTION).is_some_and(|a| a == ACTION_RESET);
    let settings = &state.usecases.settings;
    let result = if resetting {
        settings.reset(&actor, group.code(), why).await
    } else {
        // Only this group's keys: a stray field never reaches the use case.
        settings
            .update(&actor, &group_values(group, &form), why)
            .await
    };
    match result {
        Ok(()) => {
            let flash = if resetting {
                Flash::SettingsReset
            } else {
                Flash::SettingsSaved
            };
            let mut response = see_other(&tab_href(group)?)?;
            flash.set_on(&mut response);
            Ok(response)
        }
        Err(err) if is_refusal(&err) => {
            let errors = FormErrors::from_error(&state, &err);
            let view = View {
                group,
                typed: Some((&form, &errors)),
                image_error: None,
                flash: None,
                status: err.kind().status(),
            };
            page(&state, &actor, view).await
        }
        Err(err) => Err(err),
    }
}

/// POST /admin/settings/image (multipart: `key`, the photo box, `reason`):
/// the logo or the browser icon. Who is asking is checked before the body
/// is read; a refused upload re-shows the tab with its message under the
/// box.
pub async fn upload_image(
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
    let (form, file): (ImageForm, _) = upload.into_form(IMAGE_FIELD, max).await?;
    let result = match file {
        Some(bytes) => {
            settings
                .set_image(&actor, &form.key, bytes, reason(&form.reason))
                .await
        }
        None => Err(Error::new(ErrorCode::ImageRequired)),
    };
    image_done(&state, &actor, form.key, result, Flash::SettingsSaved).await
}

/// POST /admin/settings/image/delete (`key`, `reason`): back on the built-in
/// logo or icon.
pub async fn remove_image(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<ImageForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let result = state
        .usecases
        .settings
        .remove_image(&actor, &form.key, reason(&form.reason))
        .await;
    image_done(&state, &actor, form.key, result, Flash::ImageRemoved).await
}

/// Back on the identity tab with `flash`, or the tab again with the refusal
/// under the image's box.
async fn image_done(
    state: &AppState,
    actor: &Actor,
    key: String,
    result: Result<(), Error>,
    flash: Flash,
) -> Result<Response, Error> {
    let group = SettingGroup::Brand;
    match result {
        Ok(()) => {
            let mut response = see_other(&tab_href(group)?)?;
            flash.set_on(&mut response);
            Ok(response)
        }
        Err(err) if is_refusal(&err) => {
            let message = FormErrors::from_error(state, &err)
                .message
                .unwrap_or_default();
            let view = View {
                group,
                typed: None,
                image_error: Some((key, message)),
                flash: None,
                status: err.kind().status(),
            };
            page(state, actor, view).await
        }
        Err(err) => Err(err),
    }
}
