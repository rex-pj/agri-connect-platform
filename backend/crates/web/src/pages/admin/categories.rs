//! /admin/categories and /admin/categories/{id}: the category list with a
//! create form, and one category's edit form and visibility.
//!
//! One screen, two tabs (`?type=`): service types (`service_types`) and
//! supply types (`supply_types`, "vật tư"), each with its count; the create sheet makes one of the tab's type.
//!
//! Hiding a category takes its listings off the public pages, so it asks for
//! confirmation and a reason (a dialog with JS, the same form on the page
//! without). Showing it again is one button. A category never changes type.
//! The list's rows carry the same switch (`?hide={id}` opens the
//! confirmation there); sent with `back=list`, it lands back on the list.

use application::{
    constants::{CATEGORY_DESCRIPTION_MAX, CATEGORY_NAME_MAX, CATEGORY_SLUG_MAX},
    inputs::CategoryInput,
    outputs::AdminCategoryDto,
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{
    constants::{CATEGORY_TYPES, CATEGORY_TYPE_SERVICE},
    models::admin_list::LISTING_LIST_STATES,
    Error, ErrorCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::{COLOR_DEFAULT, COLOR_PRESETS},
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    labels::{sector_name, sector_options, CodeOption},
    middleware::auth::Viewer,
    pages::{
        admin::{
            catalogue_tabs, is_refusal, render_admin, url, AdminTab, CatalogueTab, ListPosition,
            ListReturn, Outcome, RowConfirm, SearchBox, BACK_TO_LIST, CATALOGUE_SORTS,
        },
        forms::{reason, whole_number, FormErrors},
        listing::{parse_opt, past_the_end, with_query, Pagination},
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
    /// The colour box's "no colour" tick (`color_none`): a colour box always
    /// sends a colour, so this is how a category has none.
    pub color_none: String,
    pub description: String,
    pub display_order: String,
    pub sector: String,
}

impl CategoryForm {
    fn input(&self) -> Result<CategoryInput, Error> {
        let color = if self.color_none.is_empty() {
            self.color.clone()
        } else {
            String::new()
        };
        Ok(CategoryInput {
            name: self.name.clone(),
            slug: self.slug.clone(),
            icon: Some(self.icon.clone()),
            color: Some(color),
            description: Some(self.description.clone()),
            display_order: whole_number(&self.display_order, "display_order")?,
            sector: self.sector.clone(),
        })
    }

    fn from_dto(c: &AdminCategoryDto) -> Self {
        Self {
            category_type: c.category_type.clone(),
            name: c.name.clone(),
            slug: c.slug.clone(),
            icon: c.icon.clone().unwrap_or_default(),
            color: c.color.clone().unwrap_or_default(),
            color_none: String::new(),
            description: c.description.clone().unwrap_or_default(),
            display_order: c.display_order.to_string(),
            sector: c.sector.clone().unwrap_or_default(),
        }
    }
}

/// The use case's length limits, for the boxes' `maxlength`, and the colour
/// box's swatches and starting colour.
#[derive(Serialize)]
pub struct Limits {
    pub name: usize,
    pub slug: usize,
    pub slug_pattern: &'static str,
    pub description: usize,
    pub color_presets: &'static [&'static str],
    pub color_default: &'static str,
}

const LIMITS: Limits = Limits {
    name: CATEGORY_NAME_MAX,
    slug: CATEGORY_SLUG_MAX,
    slug_pattern: application::validation::SLUG_PATTERN,
    description: CATEGORY_DESCRIPTION_MAX,
    color_presets: COLOR_PRESETS,
    color_default: COLOR_DEFAULT,
};

#[derive(Serialize)]
pub struct CategoryRow {
    pub category: AdminCategoryDto,
    /// Its sector, worded ("every sector" when none).
    pub sector_label: String,
    pub href: String,
    /// The list again with this row's hide confirmation open.
    pub hide_href: String,
    pub status_action: String,
}

/// The type a `?type=` names, else services.
fn tab_type(text: &str) -> &'static str {
    CATEGORY_TYPES
        .iter()
        .copied()
        .find(|t| *t == text)
        .unwrap_or(CATEGORY_TYPE_SERVICE)
}

/// The list on the tab of `kind`, at the list page `page` (a number, else
/// the first).
fn list_href(kind: &str, position: &ListPosition) -> Result<String, Error> {
    let query = position.query();
    let mut params = vec![("type", kind)];
    params.extend(query.iter().map(|(k, v)| (*k, v.as_str())));
    Ok(with_query(&url("admin_categories", &[])?, &params))
}

#[derive(Serialize)]
pub struct CategoriesPage {
    /// How many types match, for the result bar.
    pub total: i64,
    /// The tab shown and every tab.
    pub kind: &'static str,
    pub tabs: Vec<AdminTab>,
    pub limits: &'static Limits,
    /// The sectors a category may serve, for the form.
    pub sectors: Vec<CodeOption>,
    pub rows: Vec<CategoryRow>,
    pub form: CategoryForm,
    pub errors: FormErrors,
    pub create_action: String,
    pub confirm: Option<RowConfirm>,
    /// The hidden fields of every row action: land back here.
    pub keep: ListReturn,
    pub search: SearchBox,
    /// The shown / hidden chips.
    pub states: Vec<AdminTab>,
    pub pagination: Option<Pagination>,
    pub flash: Option<Flash>,
}

/// The list page around the create form; `hide`: the row whose hide
/// confirmation is open.
struct ListView {
    kind: &'static str,
    /// The list page shown (`?page=`).
    page: String,
    form: CategoryForm,
    errors: FormErrors,
    /// The words searched.
    q: String,
    sector: String,
    state: String,
    sort: String,
    gap: String,
    hide: Option<i32>,
    flash: Option<Flash>,
    status: StatusCode,
}

async fn list_page(state: &AppState, actor: &Actor, view: ListView) -> Result<Response, Error> {
    let ListView {
        kind,
        page,
        form,
        errors,
        hide,
        q,
        sector,
        state: chosen_state,
        sort,
        gap,
        flash,
        status,
    } = view;
    let asked = ListPosition {
        page,
        q,
        sector,
        state: chosen_state,
        sort,
        gap,
        ..ListPosition::default()
    }
    .checked(LISTING_LIST_STATES, CATALOGUE_SORTS);
    let criteria = asked.filter(LISTING_LIST_STATES);
    let requested = asked.page();
    let (categories, sectors) = tokio::try_join!(
        state
            .usecases
            .admin
            .list_categories_page(actor, kind, requested, &criteria),
        state.usecases.sector.named(),
    )?;
    let path = url("admin_categories", &[])?;
    let wanted = asked.query();
    let mut tab = vec![("type", kind)];
    tab.extend(
        wanted
            .iter()
            .filter(|(k, _)| *k != "page")
            .map(|(k, v)| (*k, v.as_str())),
    );
    if let Some(redirect) = past_the_end(&path, &tab, categories.page, categories.total_pages) {
        return redirect;
    }
    let tabs = catalogue_tabs(
        state,
        if kind == CATEGORY_TYPE_SERVICE {
            CatalogueTab::ServiceTypes
        } else {
            CatalogueTab::SupplyTypes
        },
    )?;
    let position = ListPosition {
        page: categories.page.to_string(),
        ..asked
    };
    let mut confirm = None;
    let mut rows = Vec::with_capacity(categories.items.len());
    for category in categories.items {
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
            sector_label: match category.sector.as_deref() {
                Some(s) => sector_name(&sectors, s),
                None => state.locale().text("sector.all"),
            },
            href: url("admin_category", &[("id", &id)])?,
            hide_href: position.href("admin_categories", &[("type", kind), ("hide", &id)])?,
            status_action,
            category,
        });
    }
    let locale = state.locale();
    let search = position
        .search_box_in("admin_categories", &[("type", kind)])?
        .picked(
            "sector",
            locale.text("sector.label"),
            locale.text("admin_pick.all_sectors"),
            sectors
                .iter()
                .map(|s| (s.code.clone(), s.name.clone()))
                .collect(),
        )
        .toggled("gap", "unused", locale.text("admin_toggle.unused"))
        .sorted(
            locale,
            &position,
            "admin_sort.display_order",
            CATALOGUE_SORTS,
        );
    let states =
        position.shared_state_chips(locale, "admin_categories", LISTING_LIST_STATES, None)?;
    let body = CategoriesPage {
        search,
        total: categories.total,
        states,
        pagination: Pagination::new(&path, &tab, categories.page, categories.total_pages),
        kind,
        tabs,
        limits: &LIMITS,
        sectors: sector_options(&sectors),
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
    /// The tab: `service` or `product`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The category whose hide confirmation is open.
    pub hide: String,
    pub page: String,
    /// The words searched, the sector, shown / hidden.
    pub q: String,
    pub sector: String,
    pub state: String,
    pub sort: String,
    pub gap: String,
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
    let kind = tab_type(&q.kind);
    let view = ListView {
        kind,
        form: CategoryForm {
            category_type: kind.to_owned(),
            ..CategoryForm::default()
        },
        errors: FormErrors::default(),
        hide: parse_opt(&q.hide),
        page: q.page,
        q: q.q,
        sector: q.sector,
        state: q.state,
        sort: q.sort,
        gap: q.gap,
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
                kind: tab_type(&form.category_type),
                page: String::new(),
                errors: FormErrors::from_error(&state, &err),
                form,
                hide: None,
                q: String::new(),
                sector: String::new(),
                state: String::new(),
                sort: String::new(),
                gap: String::new(),
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
    /// The list on this category's tab.
    pub list_href: String,
    /// What is filed under it: the admin's supply list narrowed to it, or
    /// the public list of its services.
    pub listings_href: String,
    pub sectors: Vec<CodeOption>,
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
    let (category, sectors) = tokio::try_join!(
        state.usecases.admin.get_category_detail(actor, id),
        state.usecases.sector.named(),
    )?;
    let id_text = id.to_string();
    let path = url("admin_category", &[("id", &id_text)])?;
    let listings_href = if category.category_type == CATEGORY_TYPE_SERVICE {
        url(
            "service_category",
            &[("id", &category.id.to_string()), ("slug", &category.slug)],
        )?
    } else {
        ListPosition {
            category_id: category.id.to_string(),
            ..ListPosition::default()
        }
        .href("admin_products", &[])?
    };
    let body = CategoryPage {
        listings_href,
        list_href: list_href(&category.category_type, &ListPosition::default())?,
        limits: &LIMITS,
        sectors: sector_options(&sectors),
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
    /// The list page the row was on (`ListReturn`).
    pub page: String,
    pub q: String,
    pub sector: String,
    pub state: String,
    pub sort: String,
    pub gap: String,
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
            // Back to the list on the category's own tab.
            let category = state.usecases.admin.get_category_detail(&actor, id).await?;
            let mut response = see_other(&list_href(
                &category.category_type,
                &ListPosition {
                    page: form.page,
                    q: form.q,
                    sector: form.sector,
                    state: form.state,
                    sort: form.sort,
                    gap: form.gap,
                    ..ListPosition::default()
                },
            )?)?;
            flash.set_on(&mut response);
            Ok(response)
        }
        Ok(_) => redirect_to(id, flash),
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, None, !show);
            category_page(&state, &actor, id, outcome).await
        }
        Err(err) => Err(err),
    }
}
