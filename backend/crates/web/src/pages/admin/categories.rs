//! /admin/categories and /admin/categories/{id}: the category list with a
//! create form, and one category's edit form and visibility.
//!
//! Hiding a category takes its listings off the public pages, so it asks for
//! confirmation and a reason (a dialog with JS, the same form on the page
//! without). Showing it again is one button. A category never changes type.
//! The list's rows carry the same switch (`?hide={id}` opens the
//! confirmation there); sent with `back=list`, it lands back on the list.

use application::{
    constants::{
        CATEGORY_COLOR_MAX, CATEGORY_DESCRIPTION_MAX, CATEGORY_ICON_MAX, CATEGORY_NAME_MAX,
        CATEGORY_SLUG_MAX,
    },
    inputs::CategoryInput,
    outputs::AdminCategoryDto,
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{
            is_refusal, render_admin, url, ListPosition, ListReturn, Outcome, RowConfirm,
            BACK_TO_LIST,
        },
        forms::{reason, whole_number, FormErrors},
        listing::parse_opt,
        parse_id, see_other,
    },
};

use application::authz::Actor;

/// The form as typed: every box a string, so a bad number comes back as it
/// was entered.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct CategoryForm {
    pub category_type: String,
    pub name: String,
    pub slug: String,
    pub icon: String,
    pub color: String,
    pub description: String,
    pub display_order: String,
}

impl CategoryForm {
    fn input(&self) -> Result<CategoryInput, Error> {
        Ok(CategoryInput {
            name: self.name.clone(),
            slug: self.slug.clone(),
            icon: Some(self.icon.clone()),
            color: Some(self.color.clone()),
            description: Some(self.description.clone()),
            display_order: whole_number(&self.display_order, "display_order")?,
        })
    }

    fn from_dto(c: &AdminCategoryDto) -> Self {
        Self {
            category_type: c.category_type.clone(),
            name: c.name.clone(),
            slug: c.slug.clone(),
            icon: c.icon.clone().unwrap_or_default(),
            color: c.color.clone().unwrap_or_default(),
            description: c.description.clone().unwrap_or_default(),
            display_order: c.display_order.to_string(),
        }
    }
}

/// The use case's length limits, for the boxes' `maxlength`.
#[derive(Serialize)]
pub struct Limits {
    pub name: usize,
    pub slug: usize,
    pub icon: usize,
    pub color: usize,
    pub description: usize,
}

const LIMITS: Limits = Limits {
    name: CATEGORY_NAME_MAX,
    slug: CATEGORY_SLUG_MAX,
    icon: CATEGORY_ICON_MAX,
    color: CATEGORY_COLOR_MAX,
    description: CATEGORY_DESCRIPTION_MAX,
};

#[derive(Serialize)]
pub struct CategoryRow {
    pub category: AdminCategoryDto,
    pub href: String,
    /// The list again with this row's hide confirmation open.
    pub hide_href: String,
    pub status_action: String,
}

#[derive(Serialize)]
pub struct CategoriesPage {
    pub limits: &'static Limits,
    pub rows: Vec<CategoryRow>,
    pub form: CategoryForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub confirm: Option<RowConfirm>,
    /// The hidden fields of every row action: land back here.
    pub keep: ListReturn,
    pub flash: Option<Flash>,
}

/// The list page around the create form; `hide`: the row whose hide
/// confirmation is open.
struct ListView {
    form: CategoryForm,
    errors: FormErrors,
    hide: Option<i32>,
    flash: Option<Flash>,
    status: StatusCode,
}

async fn list_page(state: &AppState, actor: &Actor, view: ListView) -> Result<Response, Error> {
    let ListView {
        form,
        errors,
        hide,
        flash,
        status,
    } = view;
    let categories = state.usecases.admin.list_categories(actor).await?;
    let position = ListPosition::default();
    let mut confirm = None;
    let mut rows = Vec::with_capacity(categories.len());
    for category in categories {
        let id = category.id.to_string();
        let status_action = url("admin_category_status", &[("id", &id)])?;
        if hide == Some(category.id) && category.is_active {
            confirm = Some(RowConfirm {
                kind: "hide",
                name: category.name.clone(),
                action: status_action.clone(),
            });
        }
        rows.push(CategoryRow {
            href: url("admin_category", &[("id", &id)])?,
            hide_href: position.href("admin_categories", &[("hide", &id)])?,
            status_action,
            category,
        });
    }
    let path = url("admin_categories", &[])?;
    let body = CategoriesPage {
        limits: &LIMITS,
        rows,
        form,
        errors,
        create_action: path.clone(),
        confirm,
        keep: ListReturn::new(position),
        flash,
    };
    render_admin(
        state,
        actor,
        &path,
        status,
        "pages/admin/categories.html",
        &body,
    )
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    /// The category whose hide confirmation is open.
    pub hide: String,
}

/// GET /admin/categories
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
        form: CategoryForm {
            category_type: "service".to_owned(),
            ..CategoryForm::default()
        },
        errors: FormErrors::default(),
        hide: parse_opt(&q.hide),
        flash: flash.0,
        status: StatusCode::OK,
    };
    let mut response = list_page(&state, &actor, view).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/categories
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<CategoryForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let result = async {
        let input = form.input()?;
        state
            .usecases
            .admin
            .create_category(&actor, form.category_type.clone(), input)
            .await
    }
    .await;
    match result {
        Ok(created) => redirect_to(created.id, Flash::CategoryCreated),
        Err(err) if is_refusal(&err) => {
            let view = ListView {
                errors: FormErrors::from_error(&state, &err),
                form,
                hide: None,
                flash: None,
                status: err.kind().status(),
            };
            list_page(&state, &actor, view).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Serialize)]
pub struct CategoryPage {
    pub limits: &'static Limits,
    pub category: AdminCategoryDto,
    pub form: CategoryForm,
    pub errors: FormErrors,
    pub update_action: String,
    pub status_action: String,
    /// The hide dialog re-opens after a refusal.
    pub confirm_open: bool,
    pub flash: Option<Flash>,
}

async fn category_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<CategoryForm>,
) -> Result<Response, Error> {
    let Outcome {
        form,
        errors,
        confirm_open,
        flash,
        status,
    } = outcome;
    let category = state.usecases.admin.get_category_detail(actor, id).await?;
    let id_text = id.to_string();
    let path = url("admin_category", &[("id", &id_text)])?;
    let body = CategoryPage {
        limits: &LIMITS,
        form: form.unwrap_or_else(|| CategoryForm::from_dto(&category)),
        category,
        errors,
        update_action: path.clone(),
        status_action: url("admin_category_status", &[("id", &id_text)])?,
        confirm_open,
        flash,
    };
    render_admin(
        state,
        actor,
        &path,
        status,
        "pages/admin/category.html",
        &body,
    )
}

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_category", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// GET /admin/categories/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::CategoryNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let mut response = category_page(&state, &actor, id, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/categories/{id}
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<CategoryForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::CategoryNotFound)?;
    let result = async {
        let input = form.input()?;
        state
            .usecases
            .admin
            .update_category(&actor, id, input)
            .await
    }
    .await;
    match result {
        Ok(_) => redirect_to(id, Flash::CategorySaved),
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            category_page(&state, &actor, id, outcome).await
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

/// POST /admin/categories/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::CategoryNotFound)?;
    let show = form.is_active == "true";
    let result = state
        .usecases
        .admin
        .toggle_category_active(&actor, id, show, reason(&form.reason))
        .await;
    let flash = if show {
        Flash::CategoryShown
    } else {
        Flash::CategoryHidden
    };
    match result {
        Ok(_) if form.back == BACK_TO_LIST => {
            ListPosition::default().back_to("admin_categories", flash)
        }
        Ok(_) => redirect_to(id, flash),
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, None, !show);
            category_page(&state, &actor, id, outcome).await
        }
        Err(err) => Err(err),
    }
}
