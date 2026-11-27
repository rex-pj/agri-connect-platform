//! The admin pages of the short lists kept by code (`CodedUseCase`): the
//! sectors (`/admin/sectors`, its own menu entry) and the alert kinds (the
//! alerts page's second tab, `/admin/alerts?tab=kinds`). One list with a
//! create sheet and an edit sheet (`EditSheet`: an entry has a name, an
//! icon and an order, too little for a page of its own); each list says
//! where its pages are and its words (`CodedPages`), the templates are
//! shared (`pages/admin/coded_list.html`, `organisms/coded_fields.html`).
//! An entry's former address (`/admin/sectors/{id}`) opens its sheet.
//!
//! An entry is never deleted (other rows name it by its code): it is
//! hidden. Hiding asks for confirmation and a reason (a dialog with JS, the
//! same form on the page without); showing again is one button. The list's
//! rows carry both (`?hide={id}` opens the confirmation there).

use application::{
    authz::Actor,
    inputs::CodedInput,
    outputs::CodedDto,
    slug::matches_search,
    usecases::{admin_usecase::HistoryOf, coded_usecase::CodedUseCase},
};
use axum::{http::StatusCode, response::Response};
use domain::{
    models::admin_list::{LISTING_LIST_STATES, LIST_STATE_SHOWN},
    Error, ErrorCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::Flash,
    labels::SECTOR_ICON_DEFAULT,
    pages::{
        admin::{
            history_rows, render_admin, url, AdminTab, Asked, EditSheet, ListPosition, ListReturn,
            RowConfirm, SearchBox, Typed,
        },
        forms::{reason, whole_number, FormErrors},
        moved_permanently, parse_id,
    },
};

/// The locale keys of a list's words.
pub struct CodedWordKeys {
    pub title: &'static str,
    /// With `{count}`.
    pub sub: &'static str,
    pub add: &'static str,
    pub none: &'static str,
    /// With `{name}`.
    pub hide_title: &'static str,
    pub what: &'static str,
    pub on: &'static str,
    pub off: &'static str,
    pub name: &'static str,
    pub name_placeholder: &'static str,
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
    /// An entry's address: its edit sheet posts there.
    pub entry_route: &'static str,
    pub status_route: &'static str,
    pub icon: &'static str,
    pub not_found: ErrorCode,
    pub words: CodedWordKeys,
    pub created: Flash,
    pub saved: Flash,
    pub hidden: Flash,
    pub shown: Flash,
    /// An entry's history, and the audit code of hiding one (the line under
    /// a hidden one: who hid it and why).
    pub history: fn(i32) -> HistoryOf,
    pub hide_action: &'static str,
}

/// The words of a list, in the visitor's language.
#[derive(Serialize)]
pub struct CodedWords {
    pub title: String,
    pub sub: String,
    pub add: String,
    pub none: String,
    pub what: String,
    pub on: String,
    pub off: String,
    pub name: String,
    pub name_placeholder: String,
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
            what: l.text(w.what),
            on: l.text(w.on),
            off: l.text(w.off),
            name: l.text(w.name),
            name_placeholder: l.text(w.name_placeholder),
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

    /// The list's position: its tab, and what a form or an address carried.
    fn position(&self, from: ListPosition) -> ListPosition {
        ListPosition {
            tab: self.list_tab.to_owned(),
            ..from
        }
        .checked(LISTING_LIST_STATES, &[])
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
    /// The list again with its edit sheet open.
    pub edit_href: String,
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
    /// The entry whose edit sheet is open.
    pub edit: Option<EditSheet<CodedDto, CodedForm>>,
    pub hide_action: &'static str,
    pub keep: ListReturn,
    pub flash_text: Option<String>,
    pub search: SearchBox,
    /// The shown / hidden chips.
    pub states: Vec<AdminTab>,
}

/// What a list render starts from: where it is, the create form as typed
/// and its errors, the row asked about (and its edit as typed after a
/// refusal), the flash.
pub struct ListView {
    /// The words searched, shown / hidden.
    pub position: ListPosition,
    pub form: CodedForm,
    pub errors: FormErrors,
    pub asked: Asked,
    pub typed: Typed<CodedForm>,
    pub flash: Option<Flash>,
    pub status: StatusCode,
}

impl ListView {
    /// The list as an address asks for it: narrowed to the entries with
    /// `q`, shown or hidden, maybe a row's confirmation or edit sheet open.
    pub fn asked(q: &ListQuery, flash: Option<Flash>) -> Self {
        Self {
            position: ListPosition {
                q: q.q.clone(),
                state: q.state.clone(),
                ..ListPosition::default()
            },
            form: CodedForm::default(),
            errors: FormErrors::default(),
            asked: Asked::new(&q.hide, "").editing(&q.edit),
            typed: None,
            flash,
            status: StatusCode::OK,
        }
    }
}

/// A list's query.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    /// The entry whose hide confirmation is open.
    pub hide: String,
    /// The entry whose edit sheet is open.
    pub edit: String,
    /// The words searched.
    pub q: String,
    /// Shown / hidden.
    pub state: String,
}

pub async fn list_page(
    pages: &CodedPages,
    state: &AppState,
    actor: &Actor,
    view: ListView,
) -> Result<Response, Error> {
    let entries = pages.uc(state).list(actor).await?;
    let total = entries.len();
    let position = pages.position(view.position);
    let search = position.search_box(pages.list_route)?;
    let edit = match view.asked.edit {
        Some(id) => Some(edit_sheet(pages, state, actor, &position, id, view.typed).await?),
        None => None,
    };
    let entries: Vec<CodedDto> = match position.search() {
        Some(q) => entries
            .into_iter()
            .filter(|e| matches_search(&[&e.name, &e.code], q))
            .collect(),
        None => entries,
    };
    let wanted = position.state_in(LISTING_LIST_STATES);
    let entries: Vec<CodedDto> = entries
        .into_iter()
        .filter(|e| wanted.is_none_or(|s| (s == LIST_STATE_SHOWN) == e.is_active))
        .collect();
    let states =
        position.shared_state_chips(state.locale(), pages.list_route, LISTING_LIST_STATES, None)?;
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
            edit_href: position.edit_href(pages.list_route, &[], entry.id)?,
            status_action,
            hide_href: position.href(pages.list_route, &[("hide", &id)])?,
            entry,
        });
    }
    let path = url(pages.list_route, &[])?;
    let body = CodedListPage {
        words: pages.words(state, total),
        states,
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
        edit,
        hide_action: pages.hide_action,
        keep: ListReturn::new(position),
        flash_text: pages.flash_text(state, view.flash),
        search,
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

/// The edit sheet of entry `id`, posting back to this list position.
async fn edit_sheet(
    pages: &CodedPages,
    state: &AppState,
    actor: &Actor,
    position: &ListPosition,
    id: i32,
    typed: Typed<CodedForm>,
) -> Result<EditSheet<CodedDto, CodedForm>, Error> {
    let (entry, history) = tokio::try_join!(
        pages.uc(state).get(actor, id),
        history_rows(state, actor, (pages.history)(id)),
    )?;
    Ok(EditSheet::new(
        entry,
        typed,
        CodedForm::from_dto,
        position.record_href(pages.entry_route, id)?,
    )
    .with_history(history))
}

/// POST of the create sheet: back on the list.
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
        Ok(_) => pages
            .position(ListPosition::default())
            .back_to(pages.list_route, pages.created),
        Err(err) if super::is_refusal(&err) => {
            let view = ListView {
                position: ListPosition::default(),
                errors: FormErrors::from_error(state, &err),
                form,
                asked: Asked::default(),
                typed: None,
                flash: None,
                status: err.kind().status(),
            };
            list_page(pages, state, actor, view).await
        }
        Err(err) => Err(err),
    }
}

/// GET of an entry's former page: its edit sheet on the list.
pub fn show(pages: &CodedPages, raw_id: &str) -> Result<Response, Error> {
    let id = parse_id(raw_id, pages.not_found)?;
    moved_permanently(&pages.position(ListPosition::default()).edit_href(
        pages.list_route,
        &[],
        id,
    )?)
}

/// POST of an entry's edit sheet, from the list position in its query.
pub async fn update(
    pages: &CodedPages,
    state: &AppState,
    actor: &Actor,
    raw_id: &str,
    from: ListPosition,
    form: CodedForm,
) -> Result<Response, Error> {
    let id = parse_id(raw_id, pages.not_found)?;
    let result = async {
        let input = form.input()?;
        pages.uc(state).update(actor, id, input).await
    }
    .await;
    match result {
        Ok(()) => pages.position(from).back_to(pages.list_route, pages.saved),
        Err(err) if super::is_refusal(&err) => {
            let view = ListView {
                position: from,
                form: CodedForm::default(),
                errors: FormErrors::default(),
                asked: Asked {
                    edit: Some(id),
                    ..Asked::default()
                },
                typed: Some((form, FormErrors::from_error(state, &err))),
                flash: None,
                status: err.kind().status(),
            };
            list_page(pages, state, actor, view).await
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
    /// The list position the row was on (`ListReturn`).
    pub q: String,
    pub state: String,
}

/// POST of hide / show: back on the list.
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
    pages
        .position(ListPosition {
            q: form.q,
            state: form.state,
            ..ListPosition::default()
        })
        .back_to(pages.list_route, flash)
}
