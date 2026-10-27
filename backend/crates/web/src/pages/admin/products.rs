//! /admin/products: every provider's products, optionally for one provider or
//! by name, with hide / show and delete; /admin/products/{id}: one product,
//! to edit, hide / show or delete.
//!
//! Hiding and deleting ask for confirmation and a reason. The row's link
//! reloads the list with `?hide={id}` or `?delete={id}`, which renders that
//! confirmation open: a dialog with JS, an anchored form without. Showing a
//! product again is one button. A row's action lands back on the same page of
//! the list (`back=list`); the product page's own actions land back on it, a
//! deletion on the provider's products.

use application::{
    authz::Actor,
    outputs::{AdminPagedResponse, AdminProductDto, CategoryDto},
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
            is_refusal, render_admin, url, ListPosition, ListReturn, Outcome, ProviderFilter,
            RowConfirm, SearchBox, BACK_TO_LIST,
        },
        canonical_url,
        forms::{reason, FormErrors},
        listing::{parse_opt, Pagination},
        parse_id,
        product_form::{product_categories, ProductForm},
        see_other,
    },
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    pub provider_id: String,
    pub q: String,
    /// The product whose hide confirmation is open.
    pub hide: String,
    /// The product whose delete confirmation is open.
    pub delete: String,
}

#[derive(Serialize)]
pub struct ProductRow {
    pub product: AdminProductDto,
    /// The product's admin page.
    pub href: String,
    pub provider_href: String,
    pub hide_href: String,
    pub delete_href: String,
    pub status_action: String,
}

#[derive(Serialize)]
pub struct ProductsPage {
    pub total: i64,
    pub rows: Vec<ProductRow>,
    pub pagination: Option<Pagination>,
    /// Set when the list is one provider's.
    pub filter: Option<ProviderFilter>,
    pub search: SearchBox,
    /// The hidden fields of every row action: land back here.
    pub keep: ListReturn,
    pub confirm: Option<RowConfirm>,
    pub flash: Option<Flash>,
}

/// GET /admin/products
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let position = ListPosition {
        page: q.page.clone(),
        provider_id: q.provider_id.clone(),
        q: q.q.clone(),
    };
    let AdminPagedResponse {
        items,
        total,
        page,
        total_pages,
        ..
    } = state
        .usecases
        .admin
        .list_products_admin(
            &actor,
            position.page(),
            position.provider_id(),
            position.search(),
        )
        .await?;
    let position = ListPosition {
        page: page.to_string(),
        ..position
    };
    let hide = parse_opt::<i32>(&q.hide);
    let delete = parse_opt::<i32>(&q.delete);
    let mut confirm = None;
    let mut rows = Vec::with_capacity(items.len());
    for product in items {
        let id = product.id.to_string();
        if hide == Some(product.id) && product.is_active {
            confirm = Some(RowConfirm {
                kind: "hide",
                name: product.name.clone(),
                action: url("admin_product_status", &[("id", &id)])?,
            });
        }
        if delete == Some(product.id) {
            confirm = Some(RowConfirm {
                kind: "delete",
                name: product.name.clone(),
                action: url("admin_product_delete", &[("id", &id)])?,
            });
        }
        rows.push(ProductRow {
            href: url("admin_product", &[("id", &id)])?,
            provider_href: url(
                "admin_provider",
                &[("id", &product.business_id.to_string())],
            )?,
            hide_href: position.href("admin_products", &[("hide", &id)])?,
            delete_href: position.href("admin_products", &[("delete", &id)])?,
            status_action: url("admin_product_status", &[("id", &id)])?,
            product,
        });
    }
    let path = url("admin_products", &[])?;
    let name = rows.first().map(|r| r.product.business_name.clone());
    let body = ProductsPage {
        total,
        filter: position.provider_filter("admin_products", name)?,
        search: position.search_box("admin_products")?,
        pagination: position.pagination(&path, page, total_pages),
        rows,
        keep: ListReturn::new(position),
        confirm,
        flash: flash.0,
    };
    let mut response = render_admin(
        &state,
        &actor,
        &path,
        StatusCode::OK,
        "pages/admin/products.html",
        &body,
    )?;
    flash.clear_on(&mut response);
    Ok(response)
}

#[derive(Serialize)]
pub struct ProductPage {
    pub product: AdminProductDto,
    pub categories: Vec<CategoryDto>,
    pub form: ProductForm,
    pub errors: FormErrors,
    pub update_action: String,
    pub status_action: String,
    pub delete_action: String,
    pub provider_href: String,
    /// The page farmers see; `None` while it is hidden (it would 404).
    pub public_href: Option<String>,
    /// The provider's products, where a deletion lands.
    pub provider_products_href: String,
    pub flash: Option<Flash>,
}

/// The provider's products in the admin list.
fn provider_products(business_id: i32) -> ListPosition {
    ListPosition {
        provider_id: business_id.to_string(),
        ..ListPosition::default()
    }
}

async fn product_page(
    state: &AppState,
    actor: &Actor,
    id: i32,
    outcome: Outcome<ProductForm>,
) -> Result<Response, Error> {
    let product = state.usecases.admin.get_product_admin(actor, id).await?;
    let id_text = id.to_string();
    let path = url("admin_product", &[("id", &id_text)])?;
    let body = ProductPage {
        form: outcome.form.unwrap_or_else(|| {
            ProductForm::stored(
                product.category_id,
                &product.name,
                product.price,
                product.unit.as_deref(),
                product.description.as_deref(),
            )
        }),
        categories: product_categories(state).await?,
        errors: outcome.errors,
        update_action: path.clone(),
        status_action: url("admin_product_status", &[("id", &id_text)])?,
        delete_action: url("admin_product_delete", &[("id", &id_text)])?,
        provider_href: url(
            "admin_provider",
            &[("id", &product.business_id.to_string())],
        )?,
        public_href: product
            .is_active
            .then(|| canonical_url("product", product.id, &product.name))
            .transpose()?,
        provider_products_href: provider_products(product.business_id)
            .href("admin_products", &[])?,
        product,
        flash: outcome.flash,
    };
    render_admin(
        state,
        actor,
        &path,
        outcome.status,
        "pages/admin/product.html",
        &body,
    )
}

fn redirect_to(id: i32, flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("admin_product", &[("id", &id.to_string())])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

/// GET /admin/products/{id}
pub async fn show(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProductNotFound)?;
    let outcome = Outcome {
        flash: flash.0,
        ..Outcome::default()
    };
    let mut response = product_page(&state, &actor, id, outcome).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /admin/products/{id}
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<ProductForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProductNotFound)?;
    let result = match form.update_input(id) {
        Ok(input) => {
            state
                .usecases
                .admin
                .update_product_admin(&actor, input)
                .await
        }
        Err(err) => Err(err),
    };
    match result {
        Ok(_) => redirect_to(id, Flash::ProductSaved),
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            product_page(&state, &actor, id, outcome).await
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
    /// `list` when sent from the list's row: land back there, at `page` /
    /// `provider_id` / `q`.
    pub back: String,
    pub page: String,
    pub provider_id: String,
    pub q: String,
}

/// POST /admin/products/{id}/status
pub async fn change_status(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<StatusForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProductNotFound)?;
    let show = form.is_active == "true";
    state
        .usecases
        .admin
        .toggle_product_active(&actor, id, show, reason(&form.reason))
        .await?;
    let flash = if show {
        Flash::ProductShown
    } else {
        Flash::ProductHidden
    };
    if form.back == BACK_TO_LIST {
        return ListPosition {
            page: form.page,
            provider_id: form.provider_id,
            q: form.q,
        }
        .back_to("admin_products", flash);
    }
    redirect_to(id, flash)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct DeleteForm {
    pub reason: String,
    /// `list` when sent from the list's row: land back there, at `page` /
    /// `provider_id` / `q`.
    pub back: String,
    pub page: String,
    pub provider_id: String,
    pub q: String,
}

/// POST /admin/products/{id}/delete: back to the list it was sent from, else
/// to the provider's products.
pub async fn delete(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<DeleteForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProductNotFound)?;
    let admin = &state.usecases.admin;
    let business_id = admin.get_product_admin(&actor, id).await?.business_id;
    admin
        .delete_product_admin(&actor, id, reason(&form.reason))
        .await?;
    let position = if form.back == BACK_TO_LIST {
        ListPosition {
            page: form.page,
            provider_id: form.provider_id,
            q: form.q,
        }
    } else {
        provider_products(business_id)
    };
    position.back_to("admin_products", Flash::ProductDeleted)
}
