//! The admin pages of the short lists kept by code (`CodedUseCase`): the
//! sectors (`/admin/sectors`, its own menu entry) and the alert kinds (the
//! alerts page's second tab, `/admin/alerts?tab=kinds`). One list with a
//! create sheet, one entry's page with its edit form and visibility; each
//! list says where its pages are and its words (`CodedPages`), the
//! templates are shared (`pages/admin/coded_list.html`,
//! `coded_entry.html`, `organisms/coded_fields.html`).
//!
//! An entry is never deleted (other rows name it by its code): it is
//! hidden. Hiding asks for confirmation and a reason (a dialog with JS, the
//! same form on the page without); showing again is one button. The list's
//! rows carry both (`?hide={id}` opens the confirmation there); sent with
//! `back=list`, they land back on the list.

use application::{
    authz::Actor, inputs::CodedInput, outputs::CodedDto, usecases::coded_usecase::CodedUseCase,
};
use axum::{http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::Flash,
    labels::SECTOR_ICON_DEFAULT,
    pages::{
        admin::{
            render_admin, url, AdminTab, Asked, ListPosition, ListReturn, Outcome, RowConfirm,
            BACK_TO_LIST,
        },
        forms::{reason, whole_number, FormErrors},
        parse_id, see_other,
    },
};

/// The locale keys of a list's words.
pub struct CodedWordKeys {
    pub title: &'static str,
    /// With `{count}`.
    pub sub: &'static str,
    pub add: &'static str,
    pub none: &'static str,
    pub hide: &'static str,
    /// With `{name}`.
    pub hide_title: &'static str,
    pub what: &'static str,
    pub on: &'static str,
    pub off: &'static str,
    pub name: &'static str,
    pub name_placeholder: &'static str,
    /// The entry page's way to what is filed under the list.
    pub related: &'static str,
    pub created: &'static str,
    pub hidden: &'static str,
    pub shown: &'static str,
}

/// The tabs of the page a list is on, the list's marked (`true`).
pub type PageTabs = fn(&AppState, bool) -> Result<Vec<AdminTab>, Error>;

/// One list's pages: its use case, its routes, its words.
pub struct CodedPages {
    pub usecase: fn(&AppState) -> &CodedUseCase,
    /// The list's address (a route, and a `?tab=` when it is a tab).
    pub list_route: &'static str,
    pub list_tab: &'static str,
    /// The tabs of the page the list is on, when it is a tab.
    pub tabs: Option<PageTabs>,
    pub create_route: &'static str,
    pub entry_route: &'static str,
    pub status_route: &'static str,
    /// The entry page's header action (what is filed under the list).
    pub related_route: &'static str,
    pub related_icon: &'static str,
    pub icon: &'static str,
    pub not_found: ErrorCode,
    pub words: CodedWordKeys,
    pub created: Flash,
    pub saved: Flash,
    pub hidden: Flash,
    pub shown: Flash,
}

/// The words of a list, in the visitor's language.
#[derive(Serialize)]
pub struct CodedWords {
    pub title: String,
    pub sub: String,
    pub add: String,
    pub none: String,
    pub hide: String,
    pub what: String,
    pub on: String,
    pub off: String,
    pub name: String,
    pub name_placeholder: String,
    pub related: String,
}

impl CodedPages {
    fn uc<'a>(&self, state: &'a AppState) -> &'a CodedUseCase {
        (self.usecase)(state)
    }

    fn words(&self, state: &AppState, count: usize) -> CodedWords {
        let l = state.locale();
        let w = &self.words;
        CodedWords {
            title: l.text(w.title),
            sub: l.text_with(w.sub, &[("count", &count)]),
            add: l.text(w.add),
            none: l.text(w.none),
            hide: l.text(w.hide),
            what: l.text(w.what),
            on: l.text(w.on),
            off: l.text(w.off),
            name: l.text(w.name),
            name_placeholder: l.text(w.name_placeholder),
            related: l.text(w.related),
        }
    }

    fn hide_title(&self, state: &AppState, name: &str) -> String {
        state
            .locale()
            .text_with(self.words.hide_title, &[("name", &name)])
    }

    /// The flash's words, when it is one of this list's.
    fn flash_text(&self, state: &AppState, flash: Option<Flash>) -> Option<String> {
        let key = match flash? {
            f if f == self.created => self.words.created,
            f if f == self.saved => "admin.saved",
            f if f == self.hidden => self.words.hidden,
            f if f == self.shown => self.words.shown,
            _ => return None,
        };
        Some(state.locale().text(key))
    }

    fn position(&self) -> ListPosition {
        ListPosition {
            tab: self.list_tab.to_owned(),
            ..ListPosition::default()
        }
    }

    fn list_href(&self) -> Result<String, Error> {
        self.position().href(self.list_route, &[])
    }

    fn redirect_to(&self, id: i32, flash: Flash) -> Result<Response, Error> {
        let mut response = see_other(&url(self.entry_route, &[("id", &id.to_string())])?)?;
        flash.set_on(&mut response);
        Ok(response)
    }
}

/// The form as typed.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct CodedForm {
    pub name: String,
    pub icon: String,
    pub display_order: String,
}

impl CodedForm {
    fn input(&self) -> Result<CodedInput, Error> {
        Ok(CodedInput {
            name: self.name.clone(),
            icon: Some(self.icon.clone()),
            display_order: whole_number(&self.display_order, "display_order")?,
        })
    }

    fn from_dto(s: &CodedDto) -> Self {
        Self {
            name: s.name.clone(),
            icon: s.icon.clone().unwrap_or_default(),
            display_order: s.display_order.to_string(),
        }
    }
}

fn icon_of(entry: &CodedDto) -> String {
    entry
        .icon
        .clone()
        .unwrap_or_else(|| SECTOR_ICON_DEFAULT.to_owned())
}

#[derive(Serialize)]
pub struct CodedRow {
    pub entry: CodedDto,
    /// Its icon, or the one an entry without its own shows.
    pub icon: String,
    pub href: String,
    pub status_action: String,
    pub hide_href: String,
}

#[derive(Serialize)]
pub struct CodedListPage {
    pub words: CodedWords,
    pub icon: &'static str,
    pub tabs: Vec<AdminTab>,
    pub rows: Vec<CodedRow>,
    pub form: CodedForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub confirm: Option<RowConfirm>,
    pub confirm_title: String,
    pub keep: ListReturn,
    pub flash_text: Option<String>,
}

/// What a list render starts from: the form as typed, its errors, the row
/// asked about, the flash.
pub struct ListView {
    pub form: CodedForm,
    pub errors: FormErrors,
    pub asked: Asked,
    pub flash: Option<Flash>,
    pub status: StatusCode,
}

impl ListView {
    /// The list as first shown, maybe with a row's hide confirmation open.
    pub fn asked(hide: &str, flash: Option<Flash>) -> Self {
        Self {
            form: CodedForm::default(),
            errors: FormErrors::default(),
            asked: Asked::new(hide, ""),
            flash,
            status: StatusCode::OK,
        }
    }
}

pub async fn list_page(
    pages: &CodedPages,
    state: &AppState,
    actor: &Actor,
    view: ListView,
) -> Result<Response, Error> {
    let entries = pages.uc(state).list(actor).await?;
    let position = pages.position();
    let mut confirm = None;
    let mut rows = Vec::with_capacity(entries.len());
    for entry in entries {
        let id = entry.id.to_string();
        let status_action = url(pages.status_route, &[("id", &id)])?;
        if view.asked.hide == Some(entry.id) && entry.is_active {
            confirm = Some(RowConfirm {
                kind: "hide",
                name: entry.name.clone(),
                action: status_action.clone(),
            });
        }
        rows.push(CodedRow {
            icon: icon_of(&entry),
            href: url(pages.entry_route, &[("id", &id)])?,
            status_action,
            hide_href: position.href(pages.list_route, &[("hide", &id)])?,
            entry,
        });
    }
    let path = url(pages.list_route, &[])?;
    let body = CodedListPage {
        words: pages.words(state, rows.len()),
        icon: pages.icon,
        tabs: match pages.tabs {
            Some(tabs) => tabs(state, true)?,
            None => Vec::new(),
        },
        rows,
        form: view.form,
        errors: view.errors,
        create_action: url(pages.create_route, &[])?,
        confirm_title: confirm
            .as_ref()
            .map(|c| pages.hide_title(state, &c.name))
            .unwrap_or_default(),
        confirm,
        keep: ListReturn::new(position),
        flash_text: pages.flash_text(state, view.flash),
    };
    render_admin(
        state,
        actor,
        &path,
        view.status,
        "pages/admin/coded_list.html",
        &body,
    )
}

/// POST of the create sheet.
pub async fn create(
    pages: &CodedPages,
    state: &AppState,
    actor: &Actor,
    form: CodedForm,
) -> Result<Response, Error> {
    let result = async {
        let input = form.input()?;
        pages.uc(state).create(actor, input).await
    }
    .await;
    match result {
        Ok(id) => pages.redirect_to(id, pages.created),
        Err(err) if super::is_refusal(&err) => {
            let view = ListView {
                errors: FormErrors::from_error(state, &err),
                form,
                asked: Asked::default(),
                flash: None,
                status: err.kind().status(),
            };
            list_page(pages, state, actor, view).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Serialize)]
pub struct CodedEntryPage {
    pub words: CodedWords,
    pub entry: CodedDto,
    pub icon: String,
    pub hide_title: String,
    pub form: CodedForm,
    pub errors: FormErrors,
    pub update_action: String,
    pub status_action: String,
    pub list_href: String,
    pub related_href: String,
    pub related_icon: &'static str,
    pub confirm_open: bool,
    pub flash_text: Option<String>,
}

async fn entry_page(
    pages: &CodedPages,
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<CodedForm>,
) -> Result<Response, Error> {
    let entry = pages.uc(state).get(actor, id).await?;
    let id_text = id.to_string();
    let path = url(pages.entry_route, &[("id", &id_text)])?;
    let body = CodedEntryPage {
        words: pages.words(state, 0),
        icon: icon_of(&entry),
        hide_title: pages.hide_title(state, &entry.name),
        form: outcome.form.unwrap_or_else(|| CodedForm::from_dto(&entry)),
        entry,
        errors: outcome.errors,
        update_action: path.clone(),
        status_action: url(pages.status_route, &[("id", &id_text)])?,
        list_href: pages.list_href()?,
        related_href: url(pages.related_route, &[])?,
        related_icon: pages.related_icon,
        confirm_open: outcome.confirm_open,
        flash_text: pages.flash_text(state, outcome.flash),
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/coded_entry.html",
        &body,
    )
}

/// GET of an entry's page.
pub async fn show(
    pages: &CodedPages,
    state: &AppState,
    actor: &Actor,
    raw_id: &str,
    flash: Option<Flash>,
) -> Result<Response, Error> {
    let id = parse_id(raw_id, pages.not_found)?;
    let outcome = Outcome {
        flash,
        ..Outcome::default()
    };
    entry_page(pages, state, actor, id, outcome).await
}

/// POST of an entry's edit form.
pub async fn update(
    pages: &CodedPages,
    state: &AppState,
    actor: &Actor,
    raw_id: &str,
    form: CodedForm,
) -> Result<Response, Error> {
    let id = parse_id(raw_id, pages.not_found)?;
    let result = async {
        let input = form.input()?;
        pages.uc(state).update(actor, id, input).await
    }
    .await;
    match result {
        Ok(()) => pages.redirect_to(id, pages.saved),
        Err(err) if super::is_refusal(&err) => {
            let outcome = Outcome::refused(state, &err, Some(form), false);
            entry_page(pages, state, actor, id, outcome).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    /// "true" to show, "false" to hide.
    pub is_active: String,
    pub reason: String,
    /// `list` when sent from the list's row: land back there.
    pub back: String,
}

/// POST of hide / show.
pub async fn change_status(
    pages: &CodedPages,
    state: &AppState,
    actor: &Actor,
    raw_id: &str,
    form: StatusForm,
) -> Result<Response, Error> {
    let id = parse_id(raw_id, pages.not_found)?;
    let show = form.is_active == "true";
    pages
        .uc(state)
        .set_active(actor, id, show, reason(&form.reason))
        .await?;
    let flash = if show { pages.shown } else { pages.hidden };
    if form.back == BACK_TO_LIST {
        return pages.position().back_to(pages.list_route, flash);
    }
    pages.redirect_to(id, flash)
}
