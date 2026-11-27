//! /admin/info-pages and /admin/info-pages/{id}: the information pages
//! (the terms, the policies, the rules, the legal entity, the contact page)
//! (`content.page.manage`).
//!
//! The list carries the create sheet; a page's own page its text, written
//! as a new version (previewed first, without saving, by the same form's
//! second button), who sees it (on, preview, off, as a feature) and its
//! place in the footer. Its versions are listed, each with its day; nothing
//! is deleted: a page is turned off.

use application::{
    authz::Actor,
    features::Stage,
    info_pages::FOOTER_GROUPS,
    inputs::{InfoPageInput, InfoRevisionInput},
    outputs::{InfoPageDto, InfoRevisionDto},
    usecases::{admin_usecase::HistoryOf, demand_usecase::vn_today},
};
use axum::{extract::State, http::StatusCode, response::Response};
use chrono::{NaiveDate, Utc};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath},
    locale::Locale,
    middleware::auth::Viewer,
    pages::{
        admin::{
            history_rows, is_refusal, render_admin, url, Badge, Confirm, HistoryRow, RecordAction,
            RecordHeader,
        },
        forms::{reason, FormErrors},
        parse_id, see_other,
    },
};

/// The text form's second button: show it as it would be, save nothing.
const INTENT_PREVIEW: &str = "preview";
/// The confirmations of a page's own page.
const SHEET_OFF: &str = "tat-trang";
const SHEET_PREVIEW: &str = "chi-quan-tri-xem";

/// A page's boxes as typed. `in_footer` is a checkbox: sent only ticked.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct InfoPageForm {
    pub code: String,
    pub title: String,
    pub icon: String,
    pub in_footer: String,
    pub footer_group: String,
    pub display_order: String,
}

impl InfoPageForm {
    fn input(&self) -> InfoPageInput {
        InfoPageInput {
            code: self.code.clone(),
            title: self.title.clone(),
            icon: self.icon.clone(),
            in_footer: !self.in_footer.is_empty(),
            footer_group: self.footer_group.clone(),
            display_order: self.display_order.trim().parse().ok(),
        }
    }

    fn from_dto(p: &InfoPageDto) -> Self {
        Self {
            code: p.code.clone(),
            title: p.title.clone(),
            icon: p.icon.clone().unwrap_or_default(),
            in_footer: if p.in_footer {
                "on".to_owned()
            } else {
                String::new()
            },
            footer_group: p.footer_group.clone(),
            display_order: p.display_order.to_string(),
        }
    }
}

/// A new version of the text as typed: the day as the date box sends it.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct TextForm {
    pub body: String,
    pub effective_on: String,
    pub change_note: String,
    /// `preview`: the second button.
    pub intent: String,
}

impl TextForm {
    fn input(&self) -> InfoRevisionInput {
        InfoRevisionInput {
            body: self.body.clone(),
            effective_on: NaiveDate::parse_from_str(self.effective_on.trim(), "%Y-%m-%d").ok(),
            change_note: self.change_note.clone(),
        }
    }
}

/// A choice of the footer group select.
#[derive(Serialize)]
pub struct GroupOption {
    pub value: &'static str,
    pub label: String,
}

fn groups(locale: &Locale) -> Vec<GroupOption> {
    FOOTER_GROUPS
        .into_iter()
        .map(|g| GroupOption {
            value: g,
            label: locale.text(&format!("info.group.{g}")),
        })
        .collect()
}

/// Its public address while a visitor (or an admin previewing) may open it.
fn public_href(page: &InfoPageDto) -> Result<Option<String>, Error> {
    if page.has_text && page.stage != Stage::Off.code() {
        Ok(Some(url("info_page", &[("code", &page.code)])?))
    } else {
        Ok(None)
    }
}

#[derive(Serialize)]
pub struct InfoPageRow {
    pub page: InfoPageDto,
    pub href: String,
    pub public_href: Option<String>,
    pub group_label: String,
}

#[derive(Serialize)]
pub struct InfoPagesPage {
    pub rows: Vec<InfoPageRow>,
    pub groups: Vec<GroupOption>,
    pub form: InfoPageForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub flash: Option<Flash>,
}

async fn list_page(
    state: &AppState,
    actor: &Actor,
    form: InfoPageForm,
    errors: FormErrors,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response, Error> {
    let locale = state.locale();
    let pages = state.usecases.info_page.list(actor).await?;
    let mut rows = Vec::with_capacity(pages.len());
    for page in pages {
        rows.push(InfoPageRow {
            href: url("admin_info_page", &[("id", &page.id.to_string())])?,
            public_href: public_href(&page)?,
            group_label: locale.text(&format!("info.group.{}", page.footer_group)),
            page,
        });
    }
    let path = url("admin_info_pages", &[])?;
    let body = InfoPagesPage {
        rows,
        groups: groups(locale),
        form,
        errors,
        create_action: path.clone(),
        flash,
    };
    render_admin(
        state,
        actor,
        &path,
        status,
        "pages/admin/info_pages.html",
        &body,
    )
}

/// GET /admin/info-pages
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let mut response = list_page(
        &state,
        &actor,
        InfoPageForm::default(),
        FormErrors::default(),
        flash.0,
        StatusCode::OK,
    )
    .await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/info-pages
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<InfoPageForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    match state.usecases.info_page.create(&actor, form.input()).await {
        Ok(id) => redirect_to(id, Flash::InfoPageCreated),
        Err(err) if is_refusal(&err) => {
            let errors = FormErrors::from_error(&state, &err);
            list_page(&state, &actor, form, errors, None, err.kind().status()).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Serialize)]
pub struct InfoPageRecord {
    pub page: InfoPageDto,
    pub header: RecordHeader,
    pub confirms: Vec<Confirm>,
    pub history: Vec<HistoryRow>,
    pub groups: Vec<GroupOption>,
    pub form: InfoPageForm,
    pub errors: FormErrors,
    pub text: TextForm,
    pub text_errors: FormErrors,
    /// Show the text as it would be (`markdown()`): after "preview", nothing
    /// saved.
    pub preview: bool,
    /// The latest to come into force first.
    pub revisions: Vec<InfoRevisionDto>,
    pub public_href: Option<String>,
    pub update_action: String,
    pub text_action: String,
    pub flash: Option<Flash>,
}

/// How a page's own page comes back: fresh, or after one of its two forms
/// was refused or previewed (its values as typed, its messages).
#[derive(Default)]
struct RecordView {
    form: Option<InfoPageForm>,
    errors: FormErrors,
    text: Option<TextForm>,
    text_errors: FormErrors,
    preview: bool,
    /// The stage confirmation to show open again.
    confirm_open: Option<&'static str>,
    flash: Option<Flash>,
    status: Option<StatusCode>,
}

/// The header's stage actions: on is one button, preview one button while
/// off; turning off, or back to preview from on, hides it from farmers and
/// asks first (with a reason).
fn stage_controls(
    locale: &Locale,
    page: &InfoPageDto,
    action: &str,
    open: Option<&'static str>,
) -> (Vec<RecordAction>, Vec<RecordAction>, Vec<Confirm>) {
    let mut actions = Vec::new();
    let mut menu = Vec::new();
    let mut confirms = Vec::new();
    let confirm = |id: &'static str, title: String, icon, button: String, stage, note: String| {
        Confirm {
            hidden_value: stage,
            ..Confirm::off(id, title, icon, action.to_owned(), button, "stage")
        }
        .noting(note)
        .opened(open == Some(id))
    };
    if page.stage != Stage::On.code() {
        actions.push(RecordAction::post(
            locale.text("info.turn_on"),
            "fa-eye",
            action.to_owned(),
            "stage",
            "on",
        ));
    }
    if page.stage == Stage::Off.code() {
        actions.push(
            RecordAction::post(
                locale.text("info.to_preview"),
                "fa-user-shield",
                action.to_owned(),
                "stage",
                "preview",
            )
            .tinted("secondary"),
        );
    }
    if page.stage == Stage::On.code() {
        menu.push(RecordAction::sheet(
            locale.text("info.to_preview"),
            "fa-user-shield",
            SHEET_PREVIEW,
            "warning",
        ));
        confirms.push(confirm(
            SHEET_PREVIEW,
            locale.text_with("info.to_preview_title", &[("name", &page.title)]),
            "fa-user-shield",
            locale.text("info.to_preview"),
            "preview",
            locale.text("info.to_preview_note"),
        ));
    }
    if page.stage != Stage::Off.code() {
        actions.push(RecordAction::sheet(
            locale.text("info.turn_off"),
            "fa-eye-slash",
            SHEET_OFF,
            "warning",
        ));
        confirms.push(confirm(
            SHEET_OFF,
            locale.text_with("info.turn_off_title", &[("name", &page.title)]),
            "fa-eye-slash",
            locale.text("admin.confirm_hide"),
            "off",
            locale.text("info.turn_off_note"),
        ));
    }
    (actions, menu, confirms)
}

async fn record_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    view: RecordView,
) -> Result<Response, Error> {
    let detail = state.usecases.info_page.get(actor, id).await?;
    let locale = state.locale();
    let id_text = id.to_string();
    let path = url("admin_info_page", &[("id", &id_text)])?;
    let page = detail.page;
    let public_href = public_href(&page)?;
    let stage_action = url("admin_info_page_stage", &[("id", &id_text)])?;
    let (mut actions, menu, confirms) =
        stage_controls(locale, &page, &stage_action, view.confirm_open);
    if let Some(href) = &public_href {
        actions.push(RecordAction::link(
            locale.text("info.open_public"),
            "fa-arrow-up-right-from-square",
            href.clone(),
        ));
    }
    let mut badges = Vec::new();
    if page.is_demo {
        badges.push(Badge::new("demo", true));
    }
    let header = RecordHeader {
        title: page.title.clone(),
        icon: page
            .icon
            .clone()
            .unwrap_or_else(|| "fa-file-lines".to_owned()),
        sub: Some(format!("/info/{}", page.code)),
        back_href: url("admin_info_pages", &[])?,
        back_label: locale.text("admin_menu.info_pages"),
        badges,
        actions,
        menu,
    };
    // A new version starts from the latest one written.
    let today = vn_today(Utc::now())?;
    let text = view.text.unwrap_or_else(|| TextForm {
        body: detail
            .revisions
            .first()
            .map(|r| r.body.clone())
            .unwrap_or_default(),
        effective_on: today.format("%Y-%m-%d").to_string(),
        ..TextForm::default()
    });
    let body = InfoPageRecord {
        header,
        confirms,
        history: history_rows(state, actor, HistoryOf::InfoPage(id)).await?,
        groups: groups(locale),
        form: view.form.unwrap_or_else(|| InfoPageForm::from_dto(&page)),
        errors: view.errors,
        text,
        text_errors: view.text_errors,
        preview: view.preview,
        revisions: detail.revisions,
        public_href,
        update_action: path.clone(),
        text_action: url("admin_info_page_text", &[("id", &id_text)])?,
        flash: view.flash,
        page,
    };
    render_admin(
        state,
        actor,
        &path,
        view.status.unwrap_or(StatusCode::OK),
        "pages/admin/info_page.html",
        &body,
    )
}

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_info_page", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// GET /admin/info-pages/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::InfoPageNotFound)?;
    let view = RecordView {
        flash: flash.0,
        ..RecordView::default()
    };
    let mut response = record_page(&state, &actor, id, view).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/info-pages/{id}: its title, icon and place in the footer.
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<InfoPageForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::InfoPageNotFound)?;
    match state
        .usecases
        .info_page
        .update(&actor, id, form.input())
        .await
    {
        Ok(()) => redirect_to(id, Flash::InfoPageSaved),
        Err(err) if is_refusal(&err) => {
            let view = RecordView {
                errors: FormErrors::from_error(&state, &err),
                form: Some(form),
                status: Some(err.kind().status()),
                ..RecordView::default()
            };
            record_page(&state, &actor, id, view).await
        }
        Err(err) => Err(err),
    }
}

/// POST /admin/info-pages/{id}/text: a new version, or (`intent=preview`)
/// the page again with the text as it would show, nothing saved.
pub async fn write_text(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(text): ApiForm<TextForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::InfoPageNotFound)?;
    if text.intent == INTENT_PREVIEW {
        let view = RecordView {
            preview: true,
            text: Some(text),
            ..RecordView::default()
        };
        return record_page(&state, &actor, id, view).await;
    }
    match state
        .usecases
        .info_page
        .add_revision(&actor, id, text.input())
        .await
    {
        Ok(()) => redirect_to(id, Flash::InfoPageTextSaved),
        Err(err) if is_refusal(&err) => {
            let view = RecordView {
                text_errors: FormErrors::from_error(&state, &err),
                text: Some(text),
                status: Some(err.kind().status()),
                ..RecordView::default()
            };
            record_page(&state, &actor, id, view).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StageForm {
    pub stage: String,
    pub reason: String,
}

/// POST /admin/info-pages/{id}/stage
pub async fn change_stage(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StageForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::InfoPageNotFound)?;
    state
        .usecases
        .info_page
        .set_stage(&actor, id, &form.stage, reason(&form.reason))
        .await?;
    redirect_to(id, Flash::InfoPageStageSaved)
}
