//! /admin/supplies: every provider's products, optionally for one provider or
//! by name, with hide / show and delete; /admin/supplies/{id}: one product,
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
    outputs::{AdminPagedResponse, AdminProductDto, CategoryDto, CommodityDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{models::product::AdminProductFilter, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::IMAGE_FIELD,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiMultipart, ApiPath, ApiQuery},
    labels::sector_label,
    middleware::auth::Viewer,
    pages::admin::sector_href,
    pages::{
        admin::{
            is_refusal, render_admin, url, ListPosition, ListReturn, Outcome, ProviderFilter,
            ReasonForm, RowConfirm, SearchBox, BACK_TO_LIST,
        },
        canonical_url,
        dashboard::{max_upload_mb, prepare},
        forms::{reason, FormErrors},
        listing::{parse_opt, Pagination},
        parse_id,
        product_form::{product_categories, product_commodities, ProductForm},
        see_other,
    },
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListQuery {
    pub page: String,
    pub provider_id: String,
    pub q: String,
    /// Those for one commodity, or of one supply type.
    pub commodity_id: String,
    pub category_id: String,
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
    /// The list narrowed to its supply type.
    pub category_href: String,
    /// Its sector's name, "every sector" for none.
    pub sector_label: String,
    pub sector_href: Option<String>,
    /// The list narrowed to the commodity it is for.
    pub commodity_href: Option<String>,
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
    /// Set when the list is one commodity's or one supply type's.
    pub narrowed: Option<ProviderFilter>,
    pub search: SearchBox,
    /// Set when the list is one provider's.
    pub create: Option<CreateForm>,
    /// The hidden fields of every row action: land back here.
    pub keep: ListReturn,
    pub confirm: Option<RowConfirm>,
    pub flash: Option<Flash>,
}

/// The bar above a list narrowed to one commodity (else one supply type):
/// its name from the rows, its admin page, and the way back to every supply.
fn narrowed(
    rows: &[ProductRow],
    commodity_id: Option<i32>,
    category_id: Option<i32>,
    list: &str,
) -> Result<Option<ProviderFilter>, Error> {
    let first = rows.first().map(|r| &r.product);
    if let Some(id) = commodity_id {
        return Ok(Some(ProviderFilter {
            icon: "fa-leaf",
            name: first.and_then(|p| p.commodity.clone()),
            href: url("admin_commodity", &[("id", &id.to_string())])?,
            clear_href: list.to_owned(),
        }));
    }
    category_id
        .map(|id| {
            Ok(ProviderFilter {
                icon: "fa-tags",
                name: first.map(|p| p.category.clone()),
                href: url("admin_category", &[("id", &id.to_string())])?,
                clear_href: list.to_owned(),
            })
        })
        .transpose()
}

/// The create form, shown when the list is one provider's: a supply always
/// belongs to a provider (e.g. one with no account met on a field visit).
#[derive(Serialize)]
pub struct CreateForm {
    pub business_id: i32,
    pub action: String,
    pub categories: Vec<CategoryDto>,
    pub commodities: Vec<CommodityDto>,
    pub form: ProductForm,
    pub errors: FormErrors,
}

/// The list as asked: its position, the confirmation to open, and the
/// create form as typed after a refusal.
struct Listing {
    query: ListQuery,
    create_form: Option<(ProductForm, FormErrors)>,
    flash: Option<Flash>,
    status: StatusCode,
}

async fn list_page(state: &AppState, actor: &Actor, asked: Listing) -> Result<Response, Error> {
    let Listing {
        query: q,
        create_form,
        flash,
        status,
    } = asked;
    let position = ListPosition {
        page: q.page.clone(),
        provider_id: q.provider_id.clone(),
        q: q.q.clone(),
        commodity_id: q.commodity_id.clone(),
        category_id: q.category_id.clone(),
        ..ListPosition::default()
    };
    let commodity_id = parse_opt::<i32>(&q.commodity_id).filter(|c| *c > 0);
    let category_id = parse_opt::<i32>(&q.category_id).filter(|c| *c > 0);
    let provider_id = position.provider_id();
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
            actor,
            position.page(),
            &AdminProductFilter {
                provider_id,
                category_id,
                commodity_id,
                search: position.search().map(str::to_owned),
            },
        )
        .await?;
    let position = ListPosition {
        page: page.to_string(),
        ..position
    };
    let hide = parse_opt::<i32>(&q.hide);
    let delete = parse_opt::<i32>(&q.delete);
    let sectors = state.usecases.sector.named().await?;
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
            category_href: ListPosition {
                category_id: product.category_id.to_string(),
                ..ListPosition::default()
            }
            .href("admin_products", &[])?,
            sector_label: sector_label(state, &sectors, product.sector.as_deref()),
            sector_href: sector_href(&sectors, product.sector.as_deref())?,
            commodity_href: product
                .commodity_id
                .map(|c| {
                    ListPosition {
                        commodity_id: c.to_string(),
                        ..ListPosition::default()
                    }
                    .href("admin_products", &[])
                })
                .transpose()?,
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
    let create = match provider_id {
        Some(business_id) => {
            let (form, errors) = create_form.unwrap_or_default();
            Some(CreateForm {
                business_id,
                action: path.clone(),
                categories: product_categories(state).await?,
                commodities: product_commodities(state).await?,
                form,
                errors,
            })
        }
        None => None,
    };
    let name = rows.first().map(|r| r.product.business_name.clone());
    let narrowed = narrowed(&rows, commodity_id, category_id, &path)?;
    let body = ProductsPage {
        total,
        narrowed,
        filter: position.provider_filter("admin_products", name)?,
        search: position.search_box("admin_products")?,
        pagination: position.pagination(&path, page, total_pages),
        rows,
        create,
        keep: ListReturn::new(position),
        confirm,
        flash,
    };
    render_admin(
        state,
        actor,
        &path,
        status,
        "pages/admin/products.html",
        &body,
    )
}

/// GET /admin/supplies
pub async fn list(
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let asked = Listing {
        query: q,
        create_form: None,
        flash: flash.0,
        status: StatusCode::OK,
    };
    let mut response = list_page(&state, &actor, asked).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CreateRequest {
    pub business_id: String,
    #[serde(flatten)]
    pub product: ProductForm,
}

/// POST /admin/supplies: a supply for the provider the list is narrowed
/// to; its page next, where a photo can be added.
pub async fn create(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(req): ApiForm<CreateRequest>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let business_id = parse_id(&req.business_id, ErrorCode::ProviderNotFound)?;
    let form = req.product;
    let result = match form.create_input() {
        Ok(input) => {
            state
                .usecases
                .admin
                .create_product_admin(&actor, business_id, input)
                .await
        }
        Err(err) => Err(err),
    };
    match result {
        Ok(created) => redirect_to(created.id, Flash::ProductCreated),
        Err(err) if is_refusal(&err) => {
            let errors = FormErrors::from_error(&state, &err);
            let asked = Listing {
                query: ListQuery {
                    provider_id: business_id.to_string(),
                    ..ListQuery::default()
                },
                create_form: Some((form, errors)),
                flash: None,
                status: err.kind().status(),
            };
            list_page(&state, &actor, asked).await
        }
        Err(err) => Err(err),
    }
}

#[derive(Serialize)]
pub struct ProductPage {
    pub product: AdminProductDto,
    pub categories: Vec<CategoryDto>,
    pub commodities: Vec<CommodityDto>,
    pub form: ProductForm,
    pub errors: FormErrors,
    pub update_action: String,
    pub status_action: String,
    pub delete_action: String,
    /// Takes its photo off (confirmation with a reason).
    pub image_delete_action: String,
    /// The photo box's size line.
    pub max_upload_mb: usize,
    pub provider_href: String,
    /// The page farmers see; `None` while it is off the public pages (it
    /// would 404).
    pub public_href: Option<String>,
    /// Its supply type's admin page, where a hidden type is shown again.
    pub category_href: String,
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
                product.commodity_id,
                &product.name,
                product.price,
                product.unit.as_deref(),
                product.description.as_deref(),
            )
        }),
        categories: product_categories(state).await?,
        commodities: product_commodities(state).await?,
        errors: outcome.errors,
        update_action: path.clone(),
        status_action: url("admin_product_status", &[("id", &id_text)])?,
        delete_action: url("admin_product_delete", &[("id", &id_text)])?,
        image_delete_action: url("admin_product_image_delete", &[("id", &id_text)])?,
        max_upload_mb: max_upload_mb(state),
        provider_href: url(
            "admin_provider",
            &[("id", &product.business_id.to_string())],
        )?,
        category_href: url(
            "admin_category",
            &[("id", &product.category_id.to_string())],
        )?,
        public_href: product
            .is_public
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

/// GET /admin/supplies/{id}
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

/// POST /admin/supplies/{id} (multipart: the boxes and an optional photo,
/// checked before anything is saved, as on the owner's form)
pub async fn update(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    upload: ApiMultipart,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProductNotFound)?;
    // Who is asking, and that the supply exists, before reading the photo.
    state.usecases.admin.get_product_admin(&actor, id).await?;
    let max = state.usecases.media.max_upload_bytes();
    let (form, file): (ProductForm, _) = upload.into_form(IMAGE_FIELD, max).await?;
    let image = match prepare(&state, file).await {
        Ok(image) => image,
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::photo_refused(&state, &err, form);
            return product_page(&state, &actor, id, outcome).await;
        }
        Err(err) => return Err(err),
    };
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
        Ok(_) => {
            if let Some(image) = image {
                state
                    .usecases
                    .media
                    .attach_product_image_admin(&actor, id, image)
                    .await?;
            }
            redirect_to(id, Flash::ProductSaved)
        }
        Err(err) if is_refusal(&err) => {
            let outcome = Outcome::refused(&state, &err, Some(form), false);
            product_page(&state, &actor, id, outcome).await
        }
        Err(err) => Err(err),
    }
}

/// POST /admin/supplies/{id}/image/delete: the photo off, the supply kept.
pub async fn remove_image(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    ApiForm(form): ApiForm<ReasonForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProductNotFound)?;
    state
        .usecases
        .media
        .remove_product_image(&actor, id, reason(&form.reason))
        .await?;
    redirect_to(id, Flash::ImageRemoved)
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

/// POST /admin/supplies/{id}/status
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
            ..ListPosition::default()
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

/// POST /admin/supplies/{id}/delete: back to the list it was sent from, else
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
            ..ListPosition::default()
        }
    } else {
        provider_products(business_id)
    };
    position.back_to("admin_products", Flash::ProductDeleted)
}
