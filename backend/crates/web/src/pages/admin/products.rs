//! /admin/products: every provider's products, optionally for one provider,
//! with hide / show and delete.
//!
//! Hiding and deleting ask for confirmation and a reason. The row's link
//! reloads the list with `?hide={id}` or `?delete={id}`, which renders that
//! confirmation open: a dialog with JS, an anchored form without. Showing a
//! product again is one button. Every action lands back on the same page of
//! the list.

use application::outputs::{AdminPagedResponse, AdminProductDto};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiPath, ApiQuery},
    middleware::auth::Viewer,
    pages::{
        admin::{render_admin, url, ListPosition},
        canonical_url,
        forms::reason,
        listing::{parse_opt, Pagination},
        parse_id, see_other,
    },
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    pub provider_id: String,
    /// The product whose hide confirmation is open.
    pub hide: String,
    /// The product whose delete confirmation is open.
    pub delete: String,
}

#[derive(Serialize)]
pub struct ProductRow {
    pub product: AdminProductDto,
    pub public_href: String,
    pub provider_href: String,
    pub hide_href: String,
    pub delete_href: String,
    pub status_action: String,
}

/// The confirmation open on this render.
#[derive(Serialize)]
pub struct Confirm {
    /// "hide" or "delete".
    pub kind: &'static str,
    pub name: String,
    pub action: String,
}

#[derive(Serialize)]
pub struct ProductsPage {
    pub total: i64,
    pub rows: Vec<ProductRow>,
    pub pagination: Option<Pagination>,
    /// Set when the list is one provider's.
    pub provider_name: Option<String>,
    pub all_href: String,
    pub position: ListPosition,
    pub confirm: Option<Confirm>,
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
        .list_products_admin(&actor, position.page(), position.provider_id())
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
            confirm = Some(Confirm {
                kind: "hide",
                name: product.name.clone(),
                action: url("admin_product_status", &[("id", &id)])?,
            });
        }
        if delete == Some(product.id) {
            confirm = Some(Confirm {
                kind: "delete",
                name: product.name.clone(),
                action: url("admin_product_delete", &[("id", &id)])?,
            });
        }
        rows.push(ProductRow {
            public_href: canonical_url("product", product.id, &product.name)?,
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
    let filter: Vec<(&str, &str)> = match position.provider_id() {
        Some(_) => vec![("provider_id", position.provider_id.trim())],
        None => Vec::new(),
    };
    let body = ProductsPage {
        total,
        provider_name: position
            .provider_id()
            .and_then(|_| rows.first().map(|r| r.product.business_name.clone())),
        pagination: Pagination::new(&path, &filter, page, total_pages),
        rows,
        all_href: path.clone(),
        position,
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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusForm {
    /// "true" to show, "false" to hide.
    pub is_active: String,
    pub reason: String,
    pub page: String,
    pub provider_id: String,
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
    ListPosition {
        page: form.page,
        provider_id: form.provider_id,
    }
    .back_to("admin_products", flash)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct DeleteForm {
    pub reason: String,
    pub page: String,
    pub provider_id: String,
}

/// POST /admin/products/{id}/delete
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
    state
        .usecases
        .admin
        .delete_product_admin(&actor, id, reason(&form.reason))
        .await?;
    ListPosition {
        page: form.page,
        provider_id: form.provider_id,
    }
    .back_to("admin_products", Flash::ProductDeleted)
}
