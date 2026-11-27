//! /admin/articles and /admin/articles/{id}: the news and the guides
//! (`content.post.manage`).
//!
//! The list (words, kind, sector, status chips) carries the create sheet,
//! which makes a draft; an article's own page edits it (a multipart form:
//! its boxes and its cover photo, the photo checked and stored before
//! anything is saved), previews its text without saving (the form's second
//! button), publishes, hides or turns it back into a draft (hiding asks
//! first, with a reason), and takes the cover off. Nothing is deleted.

use application::{
    authz::Actor,
    inputs::PostInput,
    outputs::{PostChoicesDto, PostDto},
    posts::{KIND_NEWS, POST_KINDS, POST_STATUSES, STATUS_DRAFT, STATUS_HIDDEN, STATUS_PUBLISHED},
    usecases::admin_usecase::HistoryOf,
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::IMAGE_FIELD,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiMultipart, ApiPath, ApiQuery},
    locale::Locale,
    middleware::auth::Viewer,
    pages::{
        admin::{
            history_rows, is_refusal, render_admin, url, Badge, Confirm, HistoryRow, ListPosition,
            Outcome, RecordAction, RecordHeader, SearchBox, CATALOGUE_SORTS,
        },
        canonical_url,
        dashboard::{max_upload_mb, prepare},
        forms::{reason, FormErrors},
        listing::{past_the_end, Pagination},
        parse_id, see_other,
    },
};

/// The edit form's second button: show the text as it would be.
const INTENT_PREVIEW: &str = "preview";
const SHEET_HIDE: &str = "go-bai";
const SHEET_COVER: &str = "bo-anh-bia";

/// An article's boxes as typed. `is_pinned` is a checkbox: sent only
/// ticked.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct PostForm {
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub sector: String,
    pub province_code: String,
    pub commodity_id: String,
    pub is_pinned: String,
    /// `preview`: the second button.
    #[serde(skip_serializing)]
    pub intent: String,
}

impl PostForm {
    fn input(&self) -> PostInput {
        PostInput {
            kind: self.kind.clone(),
            title: self.title.clone(),
            summary: self.summary.clone(),
            body: self.body.clone(),
            sector: self.sector.clone(),
            province_code: self.province_code.clone(),
            commodity_id: self.commodity_id.trim().parse().ok(),
            is_pinned: !self.is_pinned.is_empty(),
        }
    }

    fn from_dto(p: &PostDto) -> Self {
        Self {
            kind: p.kind.clone(),
            title: p.title.clone(),
            summary: p.summary.clone(),
            body: p.body.clone(),
            sector: p.sector.clone().unwrap_or_default(),
            province_code: p.province_code.clone().unwrap_or_default(),
            commodity_id: p.commodity_id.map(|c| c.to_string()).unwrap_or_default(),
            is_pinned: if p.is_pinned {
                "on".to_owned()
            } else {
                String::new()
            },
            intent: String::new(),
        }
    }
}

/// A choice of a select: its value and its words.
#[derive(Serialize)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

/// The form's selects.
#[derive(Serialize)]
pub struct PostChoices {
    pub kinds: Vec<Choice>,
    pub sectors: Vec<Choice>,
    pub provinces: Vec<Choice>,
    pub commodities: Vec<Choice>,
}

fn kind_word(locale: &Locale, kind: &str) -> String {
    locale.text(&format!("article.kind.{kind}"))
}

fn status_word(locale: &Locale, status: &str) -> String {
    locale.text(&format!("article.status.{status}"))
}

fn choices(locale: &Locale, c: PostChoicesDto) -> PostChoices {
    PostChoices {
        kinds: POST_KINDS
            .into_iter()
            .map(|k| Choice {
                value: k.to_owned(),
                label: kind_word(locale, k),
            })
            .collect(),
        sectors: c
            .sectors
            .into_iter()
            .map(|s| Choice {
                value: s.code,
                label: s.name,
            })
            .collect(),
        provinces: c
            .provinces
            .into_iter()
            .map(|p| Choice {
                value: p.code,
                label: p.name,
            })
            .collect(),
        commodities: c
            .commodities
            .into_iter()
            .map(|c| Choice {
                value: c.id.to_string(),
                label: c.name,
            })
            .collect(),
    }
}

#[derive(Serialize)]
pub struct PostRow {
    pub post: PostDto,
    pub kind_label: String,
    pub href: String,
}

#[derive(Serialize)]
pub struct ArticlesPage {
    pub total: i64,
    pub search: SearchBox,
    pub states: Vec<super::AdminTab>,
    pub rows: Vec<PostRow>,
    pub choices: PostChoices,
    pub form: PostForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub pagination: Option<Pagination>,
    pub flash: Option<Flash>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    pub q: String,
    pub kind: String,
    pub sector: String,
    pub state: String,
    pub sort: String,
}

struct ListView {
    position: ListPosition,
    form: PostForm,
    errors: FormErrors,
    flash: Option<Flash>,
    status: StatusCode,
}

async fn list_page(state: &AppState, actor: &Actor, view: ListView) -> Result<Response, Error> {
    let locale = state.locale();
    let asked = view
        .position
        .clone()
        .checked(&POST_STATUSES, CATALOGUE_SORTS);
    let criteria = asked.filter(&POST_STATUSES);
    let (posts, raw_choices) = tokio::try_join!(
        state.usecases.post.list(actor, asked.page(), &criteria),
        state.usecases.post.choices(actor),
    )?;
    let path = url("admin_articles", &[])?;
    let wanted_query = asked.query();
    let wanted: Vec<(&str, &str)> = wanted_query
        .iter()
        .filter(|(k, _)| *k != "page")
        .map(|(k, v)| (*k, v.as_str()))
        .collect();
    if let Some(redirect) = past_the_end(&path, &wanted, posts.page, posts.total_pages) {
        return redirect;
    }
    let position = ListPosition {
        page: posts.page.to_string(),
        ..asked
    };
    let form_choices = choices(locale, raw_choices);
    let search = position
        .search_box("admin_articles")?
        .picked(
            "kind",
            locale.text("article.field_kind"),
            locale.text("article.all_kinds"),
            form_choices
                .kinds
                .iter()
                .map(|k| (k.value.clone(), k.label.clone()))
                .collect(),
        )
        .picked(
            "sector",
            locale.text("sector.label"),
            locale.text("admin_pick.all_sectors"),
            form_choices
                .sectors
                .iter()
                .map(|s| (s.value.clone(), s.label.clone()))
                .collect(),
        )
        .sorted(
            locale,
            &position,
            "admin_sort.latest_change",
            CATALOGUE_SORTS,
        );
    let states = position.state_chips(
        "admin_articles",
        &POST_STATUSES,
        locale.text("filters.all"),
        |s| status_word(locale, s),
        None,
    )?;
    let mut rows = Vec::with_capacity(posts.items.len());
    for post in posts.items {
        rows.push(PostRow {
            kind_label: kind_word(locale, &post.kind),
            href: url("admin_article", &[("id", &post.id.to_string())])?,
            post,
        });
    }
    let form = if view.form.kind.is_empty() {
        PostForm {
            kind: KIND_NEWS.to_owned(),
            ..view.form
        }
    } else {
        view.form
    };
    let body = ArticlesPage {
        total: posts.total,
        search,
        states,
        pagination: position.pagination(&path, posts.page, posts.total_pages),
        rows,
        choices: form_choices,
        form,
        errors: view.errors,
        create_action: path.clone(),
        flash: view.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        view.status,
        "pages/admin/articles.html",
        &body,
    )
}

/// GET /admin/articles
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let view = ListView {
        position: ListPosition {
            page: q.page,
            q: q.q,
            kind: q.kind,
            sector: q.sector,
            state: q.state,
            sort: q.sort,
            ..ListPosition::default()
        },
        form: PostForm::default(),
        errors: FormErrors::default(),
        flash: flash.0,
        status: StatusCode::OK,
    };
    let mut response = list_page(&state, &actor, view).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/articles: a draft, then its own page.
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<PostForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    match state.usecases.post.create(&actor, form.input(), None).await {
        Ok(id) => redirect_to(id, Flash::ArticleCreated),
        Err(err) if is_refusal(&err) => {
            let view = ListView {
                position: ListPosition::default(),
                errors: FormErrors::from_error(&state, &err),
                form,
                flash: None,
                status: err.kind().status(),
            };
            list_page(&state, &actor, view).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Serialize)]
pub struct ArticleRecord {
    pub post: PostDto,
    pub header: RecordHeader,
    pub confirms: Vec<Confirm>,
    pub history: Vec<HistoryRow>,
    pub choices: PostChoices,
    pub form: PostForm,
    pub errors: FormErrors,
    /// Show the text as it would be (`markdown()`), nothing saved.
    pub preview: bool,
    pub status_label: String,
    pub update_action: String,
    pub max_upload_mb: usize,
    pub flash: Option<Flash>,
}

/// The header's status actions: publish (or again) and back to draft are
/// one button; taking it down asks first.
fn status_controls(
    locale: &Locale,
    post: &PostDto,
    action: &str,
    confirm_open: bool,
) -> (Vec<RecordAction>, Vec<RecordAction>, Vec<Confirm>) {
    let mut actions = Vec::new();
    let mut menu = Vec::new();
    let mut confirms = Vec::new();
    if post.status != STATUS_PUBLISHED {
        actions.push(RecordAction::post(
            locale.text("article.publish"),
            "fa-paper-plane",
            action.to_owned(),
            "status",
            STATUS_PUBLISHED,
        ));
    } else {
        actions.push(RecordAction::sheet(
            locale.text("article.hide"),
            "fa-eye-slash",
            SHEET_HIDE,
            "warning",
        ));
        confirms.push(
            Confirm {
                hidden_value: STATUS_HIDDEN,
                ..Confirm::off(
                    SHEET_HIDE,
                    locale.text_with("article.hide_title", &[("name", &post.title)]),
                    "fa-eye-slash",
                    action.to_owned(),
                    locale.text("admin.confirm_hide"),
                    "status",
                )
            }
            .saying("hide", locale.text("article.what"))
            .opened(confirm_open),
        );
    }
    if post.status != STATUS_DRAFT {
        menu.push(
            RecordAction::post(
                locale.text("article.to_draft"),
                "fa-pen-to-square",
                action.to_owned(),
                "status",
                STATUS_DRAFT,
            )
            .tinted("secondary"),
        );
    }
    (actions, menu, confirms)
}

async fn record_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<PostForm>,
    preview: bool,
) -> Result<Response, Error> {
    let (post, raw_choices) = tokio::try_join!(
        state.usecases.post.get(actor, id),
        state.usecases.post.choices(actor),
    )?;
    let locale = state.locale();
    let id_text = id.to_string();
    let path = url("admin_article", &[("id", &id_text)])?;
    let (mut actions, mut menu, mut confirms) = status_controls(
        locale,
        &post,
        &url("admin_article_status", &[("id", &id_text)])?,
        outcome.confirm_open,
    );
    if post.status == STATUS_PUBLISHED {
        actions.push(RecordAction::link(
            locale.text("article.open_public"),
            "fa-arrow-up-right-from-square",
            canonical_url("article", post.id, &post.title)?,
        ));
    }
    if post.cover_file_id.is_some() {
        menu.push(RecordAction::sheet(
            locale.text("article.remove_cover"),
            "fa-image",
            SHEET_COVER,
            "danger",
        ));
        confirms.push(
            Confirm::delete(
                SHEET_COVER,
                locale.text("article.remove_cover_title"),
                "fa-image",
                url("admin_article_cover_delete", &[("id", &id_text)])?,
                locale.text("article.remove_cover"),
            )
            .noting(locale.text("article.remove_cover_note")),
        );
    }
    let mut badges = Vec::new();
    if post.is_demo {
        badges.push(Badge::new("demo", true));
    }
    let header = RecordHeader {
        title: post.title.clone(),
        icon: "fa-newspaper".to_owned(),
        sub: Some(kind_word(locale, &post.kind)),
        back_href: url("admin_articles", &[])?,
        back_label: locale.text("admin_menu.articles"),
        badges,
        actions,
        menu,
    };
    let body = ArticleRecord {
        header,
        confirms,
        history: history_rows(state, actor, HistoryOf::Post(id)).await?,
        choices: choices(locale, raw_choices),
        form: outcome.form.unwrap_or_else(|| PostForm::from_dto(&post)),
        errors: outcome.errors,
        preview,
        status_label: status_word(locale, &post.status),
        update_action: path.clone(),
        max_upload_mb: max_upload_mb(state),
        flash: outcome.flash,
        post,
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/article.html",
        &body,
    )
}

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_article", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// GET /admin/articles/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PostNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let mut response = record_page(&state, &actor, id, outcome, false).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/articles/{id}: its boxes and its cover photo, or
/// (`intent=preview`) the page again with the text as it would show.
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    upload: ApiMultipart,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PostNotFound)?;
    // Who is asking, and that the article exists, before reading the photo.
    state.usecases.post.get(&actor, id).await?;
    let max = state.usecases.media.max_upload_bytes();
    let (form, file): (PostForm, _) = upload.into_form(IMAGE_FIELD, max).await?;
    if form.intent == INTENT_PREVIEW {
        let outcome = Outcome {
            form: Some(form),
            ..Outcome::default()
        };
        return record_page(&state, &actor, id, outcome, true).await;
    }
    let cover = match prepare(&state, file).await {
        Ok(cover) => cover,
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::photo_refused(&state, &err, form);
            return record_page(&state, &actor, id, outcome, false).await;
        }
        Err(err) => return Err(err),
    };
    match state
        .usecases
        .post
        .update(&actor, id, form.input(), cover)
        .await
    {
        Ok(()) => redirect_to(id, Flash::ArticleSaved),
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            record_page(&state, &actor, id, outcome, false).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    pub status: String,
    pub reason: String,
}

/// POST /admin/articles/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PostNotFound)?;
    state
        .usecases
        .post
        .set_status(&actor, id, &form.status, reason(&form.reason))
        .await?;
    redirect_to(id, Flash::ArticleStatusSaved)
}

/// POST /admin/articles/{id}/cover/delete
pub async fn remove_cover(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<super::ReasonForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::PostNotFound)?;
    state
        .usecases
        .post
        .remove_cover(&actor, id, reason(&form.reason))
        .await?;
    redirect_to(id, Flash::ArticleSaved)
}
