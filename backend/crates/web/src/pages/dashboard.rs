//! The provider's own area: /provider/dashboard (business profile, the
//! service and product lists), /provider/products/new and
//! /provider/products/{id}/edit. The service forms are in `own_services`.
//!
//! Product forms are `multipart/form-data` because they carry the photo;
//! they work without JS. The photo is checked before anything is saved
//! (`MediaUseCase::prepare_image`), so a bad one refuses the whole form
//! instead of leaving a product half-saved. The profile's location can be
//! typed or filled by the `ac-geolocate` island.

use application::{
    authz::Actor,
    catalogue::{
        PERM_OWN_INQUIRY_MANAGE, PERM_OWN_LISTING_CREATE, PERM_OWN_LISTING_DELETE,
        PERM_OWN_PROFILE_UPDATE, PERM_OWN_SERVICE_CREATE, PERM_OWN_SERVICE_DELETE,
        PERM_OWN_SERVICE_UPDATE,
    },
    constants::{CALL_STATS_DAYS, SERVICES_PER_BUSINESS_MAX},
    inputs::{CreateProductInput, UpdateProductInput, UpdateProfileInput},
    outputs::{CategoryDto, ProviderProductDto, ProviderProfileDto, ProviderServiceDto},
    ports::ProcessedImage,
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{constants::CATEGORY_TYPE_PRODUCT, Error, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    constants::IMAGE_FIELD,
    cookies::{Flash, TakeFlash},
    extract::{ApiForm, ApiMultipart, ApiPath},
    middleware::auth::Viewer,
    pages::{
        admin::is_refusal,
        canonical_url,
        forms::{optional_coordinate, vnd_amount, FormErrors},
        listing::parse_opt,
        parse_id, see_other,
    },
    routes,
    shell::Shell,
    views::render_page_status,
};

pub(super) fn url(name: &str, params: &[(&str, &str)]) -> Result<String, Error> {
    routes::url(name, params).map_err(|e| Error::internal(format!("dashboard links: {e}")))
}

pub(super) fn to_dashboard(flash: Flash) -> Result<Response, Error> {
    let mut response = see_other(&url("provider_dashboard", &[])?)?;
    flash.set_on(&mut response);
    Ok(response)
}

// ── Dashboard and profile ───────────────────────────────────────────────────

/// The profile form as typed.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProfileForm {
    pub name: String,
    pub phone: String,
    pub address: String,
    pub description: String,
    pub lat: String,
    pub lng: String,
}

impl ProfileForm {
    fn from_dto(p: &ProviderProfileDto) -> Self {
        let coord = |v: Option<f64>| v.map(|v| v.to_string()).unwrap_or_default();
        Self {
            name: p.name.clone(),
            phone: p.phone.clone(),
            address: p.address.clone().unwrap_or_default(),
            description: p.description.clone().unwrap_or_default(),
            lat: coord(p.lat),
            lng: coord(p.lng),
        }
    }

    fn input(&self) -> UpdateProfileInput {
        UpdateProfileInput {
            name: self.name.clone(),
            phone: self.phone.clone(),
            description: Some(self.description.clone()),
            address: Some(self.address.clone()),
            lat: optional_coordinate(&self.lat),
            lng: optional_coordinate(&self.lng),
        }
    }
}

#[derive(Serialize)]
pub struct ProductRow {
    pub product: ProviderProductDto,
    pub edit_href: String,
    pub public_href: String,
}

#[derive(Serialize)]
pub struct ServiceRow {
    pub service: ProviderServiceDto,
    /// `None` when the actor may neither edit nor delete it.
    pub edit_href: Option<String>,
}

/// The services section: the rows, and whether another may be added.
#[derive(Serialize)]
pub struct ServicesSection {
    pub rows: Vec<ServiceRow>,
    pub new_href: String,
    /// Allowed, and the business is under the cap.
    pub can_create: bool,
    /// At the cap (the "add" button gives way to a note).
    pub full: bool,
    pub max: u64,
}

#[derive(Serialize)]
pub struct InboxLink {
    pub href: String,
    pub total: i64,
    /// Not called back yet.
    pub new_count: i64,
}

#[derive(Serialize)]
pub struct DashboardPage {
    /// `None` when the account manages no business yet.
    pub profile: Option<ProviderProfileDto>,
    pub public_href: Option<String>,
    pub form: ProfileForm,
    pub errors: FormErrors,
    pub profile_action: String,
    pub rows: Vec<ProductRow>,
    pub new_href: String,
    pub services: ServicesSection,
    /// Services come first unless the business lists only products: the
    /// people to reach first are service providers.
    pub services_first: bool,
    pub can_edit_profile: bool,
    pub can_create: bool,
    /// The link to the inquiry inbox, when the actor may read it.
    pub inbox: Option<InboxLink>,
    /// Taps on this business's call buttons in the last `call_days` days;
    /// `None` without a business.
    pub calls: Option<i64>,
    pub call_days: i64,
    pub flash: Option<Flash>,
}

async fn dashboard_page(
    state: &AppState,
    shell: &Shell,
    actor: &Actor,
    typed: Option<(ProfileForm, FormErrors)>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response, Error> {
    let usecase = &state.usecases.provider_dashboard;
    let profile = match usecase.get_profile(actor).await {
        Ok(p) => Some(p),
        Err(e) if e.code() == ErrorCode::AccountNotLinkedToBusiness => None,
        Err(e) => return Err(e),
    };
    let mut rows = Vec::new();
    let mut service_rows = Vec::new();
    if profile.is_some() {
        let (products, services) =
            tokio::try_join!(usecase.list_products(actor), usecase.list_services(actor))?;
        for product in products {
            rows.push(ProductRow {
                edit_href: url("provider_product_edit", &[("id", &product.id.to_string())])?,
                public_href: canonical_url("product", product.id, &product.name)?,
                product,
            });
        }
        let can_manage = actor.can(PERM_OWN_SERVICE_UPDATE) || actor.can(PERM_OWN_SERVICE_DELETE);
        for service in services {
            service_rows.push(ServiceRow {
                edit_href: can_manage
                    .then(|| url("provider_service_edit", &[("id", &service.id.to_string())]))
                    .transpose()?,
                service,
            });
        }
    }
    let full = service_rows.len() as u64 >= SERVICES_PER_BUSINESS_MAX;
    let services_first = !service_rows.is_empty() || rows.is_empty();
    let services = ServicesSection {
        can_create: actor.can(PERM_OWN_SERVICE_CREATE) && !full,
        full,
        max: SERVICES_PER_BUSINESS_MAX,
        new_href: url("provider_service_new", &[])?,
        rows: service_rows,
    };
    let (form, errors) = match typed {
        Some(t) => t,
        None => (
            profile
                .as_ref()
                .map(ProfileForm::from_dto)
                .unwrap_or_default(),
            FormErrors::default(),
        ),
    };
    let inbox = match (&profile, actor.can(PERM_OWN_INQUIRY_MANAGE)) {
        (Some(_), true) => {
            let (total, new_count) = usecase.count_own_inquiries(actor).await?;
            Some(InboxLink {
                href: url("provider_inquiries", &[])?,
                total,
                new_count,
            })
        }
        _ => None,
    };
    let body = DashboardPage {
        public_href: profile
            .as_ref()
            .map(|p| canonical_url("provider", p.id, &p.name))
            .transpose()?,
        profile,
        form,
        errors,
        profile_action: url("provider_profile", &[])?,
        rows,
        new_href: url("provider_product_new", &[])?,
        services,
        services_first,
        can_edit_profile: actor.can(PERM_OWN_PROFILE_UPDATE),
        can_create: actor.can(PERM_OWN_LISTING_CREATE),
        inbox,
        calls: usecase.count_own_calls(actor).await?,
        call_days: CALL_STATS_DAYS,
        flash,
    };
    render_page_status(state, shell, status, "pages/dashboard.html", &body)
}

/// GET /provider/dashboard
pub async fn show(
    shell: Shell,
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let mut response =
        dashboard_page(&state, &shell, &actor, None, flash.0, StatusCode::OK).await?;
    flash.clear_on(&mut response);
    Ok(response)
}

/// POST /provider/profile
pub async fn update_profile(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiForm(form): ApiForm<ProfileForm>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let result = state
        .usecases
        .provider_dashboard
        .update_profile(&actor, form.input())
        .await;
    match result {
        Ok(_) => to_dashboard(Flash::ProfileSaved),
        Err(err) if is_refusal(&err) => {
            let errors = FormErrors::from_error(&state, &err);
            let status = err.kind().status();
            dashboard_page(&state, &shell, &actor, Some((form, errors)), None, status).await
        }
        Err(err) => Err(err),
    }
}

// ── Product forms ───────────────────────────────────────────────────────────

/// The product form as typed (the photo arrives separately).
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProductForm {
    pub category_id: String,
    pub name: String,
    pub price: String,
    pub unit: String,
    pub description: String,
}

/// The form's values, parsed for the use case.
struct ProductValues {
    category_id: i32,
    name: String,
    price: Option<i64>,
    unit: Option<String>,
    description: Option<String>,
}

impl ProductForm {
    fn from_dto(p: &ProviderProductDto) -> Self {
        Self {
            category_id: p.category_id.to_string(),
            name: p.name.clone(),
            price: p.price.map(|v| v.to_string()).unwrap_or_default(),
            unit: p.unit.clone().unwrap_or_default(),
            description: p.description.clone().unwrap_or_default(),
        }
    }

    /// An unparsable category becomes 0, which the use case reports on that
    /// field.
    fn values(&self) -> Result<ProductValues, Error> {
        Ok(ProductValues {
            category_id: parse_opt(&self.category_id).unwrap_or(0),
            name: self.name.clone(),
            price: vnd_amount(&self.price, "price")?,
            unit: Some(self.unit.clone()),
            description: Some(self.description.clone()),
        })
    }
}

#[derive(Serialize)]
pub struct ProductPage {
    /// `None` on the create page.
    pub product: Option<ProviderProductDto>,
    pub categories: Vec<CategoryDto>,
    pub form: ProductForm,
    pub errors: FormErrors,
    pub action: String,
    pub delete_action: Option<String>,
    pub dashboard_href: String,
    pub max_upload_mb: usize,
}

async fn product_page(
    state: &AppState,
    shell: &Shell,
    actor: &Actor,
    product: Option<ProviderProductDto>,
    typed: Option<(ProductForm, FormErrors)>,
    status: StatusCode,
) -> Result<Response, Error> {
    let (action, delete_action) = match &product {
        Some(p) => {
            let id = p.id.to_string();
            (
                url("provider_product_edit", &[("id", &id)])?,
                actor
                    .can(PERM_OWN_LISTING_DELETE)
                    .then(|| url("provider_product_delete", &[("id", &id)]))
                    .transpose()?,
            )
        }
        None => (url("provider_product_new", &[])?, None),
    };
    let (form, errors) = typed.unwrap_or_else(|| {
        (
            product
                .as_ref()
                .map(ProductForm::from_dto)
                .unwrap_or_default(),
            FormErrors::default(),
        )
    });
    let body = ProductPage {
        categories: state
            .usecases
            .explore
            .get_categories(Some(CATEGORY_TYPE_PRODUCT))
            .await?,
        product,
        form,
        errors,
        action,
        delete_action,
        dashboard_href: url("provider_dashboard", &[])?,
        max_upload_mb: max_upload_mb(state),
    };
    render_page_status(state, shell, status, "pages/dashboard_product.html", &body)
}

/// The photo, checked and re-encoded, or the form's refusal of it.
pub(super) async fn prepare(
    state: &AppState,
    upload: Option<Vec<u8>>,
) -> Result<Option<ProcessedImage>, Error> {
    match upload {
        Some(bytes) => state.usecases.media.prepare_image(bytes).await.map(Some),
        None => Ok(None),
    }
}

/// A refused form: re-shown with the typed values (a file box never keeps
/// its file, so the photo must be chosen again).
async fn refused(
    state: &AppState,
    shell: &Shell,
    actor: &Actor,
    product: Option<ProviderProductDto>,
    form: ProductForm,
    err: Error,
    errors: FormErrors,
) -> Result<Response, Error> {
    if !is_refusal(&err) {
        return Err(err);
    }
    let status = err.kind().status();
    product_page(state, shell, actor, product, Some((form, errors)), status).await
}

/// GET /provider/products/new
pub async fn new_product(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    product_page(&state, &shell, &actor, None, None, StatusCode::OK).await
}

/// POST /provider/products/new (multipart)
pub async fn create_product(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    upload: ApiMultipart,
) -> Result<Response, Error> {
    // Before reading the body: an anonymous visitor never gets to upload.
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let max = state.usecases.media.max_upload_bytes();
    let (form, file): (ProductForm, _) = upload.into_form(IMAGE_FIELD, max).await?;
    let image = match prepare(&state, file).await {
        Ok(image) => image,
        Err(err) => {
            let errors = FormErrors::on_field(&state, &err, IMAGE_FIELD);
            return refused(&state, &shell, &actor, None, form, err, errors).await;
        }
    };
    let created = async {
        let v = form.values()?;
        state
            .usecases
            .provider_dashboard
            .create_product(
                &actor,
                CreateProductInput {
                    category_id: v.category_id,
                    name: v.name,
                    description: v.description,
                    price: v.price,
                    unit: v.unit,
                },
                image,
            )
            .await
    }
    .await;
    if let Err(err) = created {
        let errors = FormErrors::from_error(&state, &err);
        return refused(&state, &shell, &actor, None, form, err, errors).await;
    }
    to_dashboard(Flash::ProductCreated)
}

/// GET /provider/products/{id}/edit
pub async fn edit_product(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProductNotFound)?;
    let product = state
        .usecases
        .provider_dashboard
        .get_product(&actor, id)
        .await?;
    product_page(&state, &shell, &actor, Some(product), None, StatusCode::OK).await
}

/// POST /provider/products/{id}/edit (multipart)
pub async fn update_product(
    shell: Shell,
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
    upload: ApiMultipart,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProductNotFound)?;
    let dashboard = &state.usecases.provider_dashboard;
    // Ownership before reading or decoding the photo.
    let current = dashboard.get_product(&actor, id).await?;
    let max = state.usecases.media.max_upload_bytes();
    let (form, file): (ProductForm, _) = upload.into_form(IMAGE_FIELD, max).await?;
    let image = match prepare(&state, file).await {
        Ok(image) => image,
        Err(err) => {
            let errors = FormErrors::on_field(&state, &err, IMAGE_FIELD);
            return refused(&state, &shell, &actor, Some(current), form, err, errors).await;
        }
    };
    let updated = async {
        let v = form.values()?;
        dashboard
            .update_product(
                &actor,
                UpdateProductInput {
                    product_id: id,
                    category_id: v.category_id,
                    name: v.name,
                    description: v.description,
                    price: v.price,
                    unit: v.unit,
                },
            )
            .await
    }
    .await;
    if let Err(err) = updated {
        let errors = FormErrors::from_error(&state, &err);
        return refused(&state, &shell, &actor, Some(current), form, err, errors).await;
    }
    if let Some(image) = image {
        state
            .usecases
            .media
            .attach_product_image(&actor, id, image)
            .await?;
    }
    to_dashboard(Flash::ProductSaved)
}

/// POST /provider/products/{id}/delete
pub async fn delete_product(
    Viewer(viewer): Viewer,
    State(state): State<AppState>,
    ApiPath(raw_id): ApiPath<String>,
) -> Result<Response, Error> {
    let Some(actor) = viewer else {
        return see_other(&url("login", &[])?);
    };
    let id = parse_id(&raw_id, ErrorCode::ProductNotFound)?;
    state
        .usecases
        .provider_dashboard
        .delete_product(&actor, id)
        .await?;
    to_dashboard(Flash::ProductDeleted)
}

/// The upload cap in whole megabytes, for the photo box's hint.
pub(super) fn max_upload_mb(state: &AppState) -> usize {
    state
        .usecases
        .media
        .max_upload_bytes()
        .div_ceil(1024 * 1024)
}
