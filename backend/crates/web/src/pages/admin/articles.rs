//! /admin/articles, /admin/articles/new and /admin/articles/{id}: the news
//! and the guides (`content.post.manage`).
//!
//! The list (words, kind, sector, status chips) leads to the editor. The
//! editor is one page for a new article and an existing one: the title
//! large with the address it makes, the summary and the text (the Markdown
//! editor) on the left; publishing, the cover photo and what the article
//! concerns on the right, below on a phone, with the save buttons kept in
//! reach. Its form is multipart (the cover is checked and stored before
//! anything is saved); "save and publish" does both at once; the preview
//! button shows the text as it would be, saving nothing (the editor's own
//! preview with JS). An existing article's header publishes, takes down
//! (with a reason) or turns it back into a draft, and takes the cover off.
//! Nothing is deleted.

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
            history_rows, is_refusal, render_admin, url, AdminTab, Badge, Confirm, HistoryRow,
            ListPosition, Outcome, ReasonForm, RecordAction, RecordHeader, SearchBox,
            CATALOGUE_SORTS,
        },
        canonical_url,
        dashboard::{max_upload_mb, prepare},
        forms::{reason, FormErrors},
        listing::{past_the_end, Pagination},
        parse_id, see_other,
    },
    slug::slugify,
};

/// The form's buttons besides "save": show the text as it would be; save
/// and publish at once.
const INTENT_PREVIEW: &str = "preview";
const INTENT_PUBLISH: &str = "publish";
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
    /// Which button sent it: `preview`, `publish`, else a plain save.
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

    /// A new article's boxes: the news, everything else blank.
    fn new_article() -> Self {
        Self {
            kind: KIND_NEWS.to_owned(),
            ..Self::default()
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
    let pairs = |items: Vec<(String, String)>| {
        items
            .into_iter()
            .map(|(value, label)| Choice { value, label })
            .collect()
    };
    PostChoices {
        kinds: POST_KINDS
            .into_iter()
            .map(|k| Choice {
                value: k.to_owned(),
                label: kind_word(locale, k),
            })
            .collect(),
        sectors: pairs(c.sectors.into_iter().map(|s| (s.code, s.name)).collect()),
        provinces: pairs(c.provinces.into_iter().map(|p| (p.code, p.name)).collect()),
        commodities: pairs(
            c.commodities
                .into_iter()
                .map(|c| (c.id.to_string(), c.name))
                .collect(),
        ),
    }
}

// ── The list ─────────────────────────────────────────────────────────────────

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
    pub states: Vec<AdminTab>,
    pub rows: Vec<PostRow>,
    pub new_href: String,
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
    let locale = state.locale();
    let asked = ListPosition {
        page: q.page,
        q: q.q,
        kind: q.kind,
        sector: q.sector,
        state: q.state,
        sort: q.sort,
        ..ListPosition::default()
    }
    .checked(&POST_STATUSES, CATALOGUE_SORTS);
    let criteria = asked.filter(&POST_STATUSES);
    let (posts, raw_choices) = tokio::try_join!(
        state.usecases.post.list(&actor, asked.page(), &criteria),
        state.usecases.post.choices(&actor),
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
    let to_pairs = |items: &[Choice]| {
        items
            .iter()
            .map(|c| (c.value.clone(), c.label.clone()))
            .collect()
    };
    let search = position
        .search_box("admin_articles")?
        .picked(
            "kind",
            locale.text("article.field_kind"),
            locale.text("article.all_kinds"),
            to_pairs(&form_choices.kinds),
        )
        .picked(
            "sector",
            locale.text("sector.label"),
            locale.text("admin_pick.all_sectors"),
            to_pairs(&form_choices.sectors),
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
    let body = ArticlesPage {
        total: posts.total,
        search,
        states,
        pagination: position.pagination(&path, posts.page, posts.total_pages),
        rows,
        new_href: url("admin_article_new", &[])?,
        flash: flash.0,
    };
    let mut response = render_admin(
        &state,
        &actor,
        &path,
        StatusCode::OK,
        "pages/admin/articles.html",
        &body,
    )?;
    flash.clear_on(&mut response);
    Ok(response)
}

// ── The editor ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ArticleEditor {
    /// `None` while writing a new article.
    pub post: Option<PostDto>,
    /// An existing article's header (its states and actions); a new one has
    /// the plain admin header.
    pub header: Option<RecordHeader>,
    pub confirms: Vec<Confirm>,
    pub history: Vec<HistoryRow>,
    pub choices: PostChoices,
    pub form: PostForm,
    pub errors: FormErrors,
    /// Show the text as it would be (`markdown()`), nothing saved.
    pub preview: bool,
    /// Where the form posts: the list (new) or the article (existing).
    pub action: String,
    pub back_href: String,
    /// The address up to the slug (`/articles/12/`), or what stands for it
    /// before the article exists; the slug of its title so far.
    pub address_prefix: String,
    pub address_slug: String,
    pub slug_url: String,
    pub preview_url: String,
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

/// The editor of the article `id`, or of a new one (`None`).
async fn editor_page(
    state: &AppState,
    actor: &Actor,
    id: Option<i32>,
    outcome: Outcome<PostForm>,
    preview: bool,
) -> Result<Response, Error> {
    let locale = state.locale();
    let raw_choices = state.usecases.post.choices(actor).await?;
    let post = match id {
        Some(id) => Some(state.usecases.post.get(actor, id).await?),
        None => None,
    };
    let mut confirms = Vec::new();
    let (header, history, action, address_prefix, path) = match &post {
        Some(post) => {
            let id_text = post.id.to_string();
            let path = url("admin_article", &[("id", &id_text)])?;
            let (mut actions, mut menu, status_confirms) = status_controls(
                locale,
                post,
                &url("admin_article_status", &[("id", &id_text)])?,
                outcome.confirm_open,
            );
            confirms = status_confirms;
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
                sub: Some(format!(
                    "{} · {}",
                    kind_word(locale, &post.kind),
                    status_word(locale, &post.status)
                )),
                back_href: url("admin_articles", &[])?,
                back_label: locale.text("admin_menu.articles"),
                badges,
                actions,
                menu,
            };
            let history = history_rows(state, actor, HistoryOf::Post(post.id)).await?;
            let prefix = format!("{}/{id_text}/", url("articles", &[])?);
            (Some(header), history, path.clone(), prefix, path)
        }
        None => {
            let path = url("admin_article_new", &[])?;
            let prefix = format!(
                "{}/{}/",
                url("articles", &[])?,
                locale.text("article.new_id")
            );
            (None, Vec::new(), url("admin_articles", &[])?, prefix, path)
        }
    };
    let form = outcome.form.unwrap_or_else(|| match &post {
        Some(p) => PostForm::from_dto(p),
        None => PostForm::new_article(),
    });
    let body = ArticleEditor {
        address_slug: slugify(&form.title),
        header,
        confirms,
        history,
        choices: choices(locale, raw_choices),
        form,
        errors: outcome.errors,
        preview,
        action,
        back_href: url("admin_articles", &[])?,
        address_prefix,
        slug_url: url("admin_article_slug", &[])?,
        preview_url: url("admin_markdown_preview", &[])?,
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

/// GET /admin/articles/new
pub async fn new(Viewer(viewer): Viewer, State(state): State<AppState>) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    editor_page(&state, &actor, None, Outcome::default(), false).await
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
    let mut response = editor_page(&state, &actor, Some(id), outcome, false).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// Saves the editor of the article `id` (`None`: a new one): the preview
/// button re-shows the page, else the cover is checked, the boxes saved and,
/// for "save and publish", the article published.
async fn save(
    state: AppState,
    actor: Actor,
    id: Option<i32>,
    upload: ApiMultipart,
) -> Result<Response, Error> {
    let max = state.usecases.media.max_upload_bytes();
    let (form, file): (PostForm, _) = upload.into_form(IMAGE_FIELD, max).await?;
    if form.intent == INTENT_PREVIEW {
        let outcome = Outcome {
            form: Some(form),
            ..Outcome::default()
        };
        return editor_page(&state, &actor, id, outcome, true).await;
    }
    let cover = match prepare(&state, file).await {
        Ok(cover) => cover,
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::photo_refused(&state, &err, form);
            return editor_page(&state, &actor, id, outcome, false).await;
        }
        Err(err) => return Err(err),
    };
    let posts = &state.usecases.post;
    let saved = match id {
        Some(id) => posts
            .update(&actor, id, form.input(), cover)
            .await
            .map(|()| id),
        None => posts.create(&actor, form.input(), cover).await,
    };
    match saved {
        Ok(saved_id) => {
            if form.intent == INTENT_PUBLISH {
                posts
                    .set_status(&actor, saved_id, STATUS_PUBLISHED, None)
                    .await?;
                return redirect_to(saved_id, Flash::ArticleStatusSaved);
            }
            let flash = if id.is_some() {
                Flash::ArticleSaved
            } else {
                Flash::ArticleCreated
            };
            redirect_to(saved_id, flash)
        }
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            editor_page(&state, &actor, id, outcome, false).await
        }
        Err(err) => Err(err),
    }
}

/// POST /admin/articles: a new article, a draft unless "save and publish".
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    upload: ApiMultipart,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    // Who is asking, before reading the photo.
    state.usecases.post.choices(&actor).await?;
    save(state, actor, None, upload).await
}

/// POST /admin/articles/{id}
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
    save(state, actor, Some(id), upload).await
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
    ApiForm(form): ApiForm<ReasonForm>,
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
