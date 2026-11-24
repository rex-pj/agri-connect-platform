//! GET /supplies/{id} and /supplies/{id}/{slug} — a supply ("vật tư") page.
//!
//! One canonical URL per product, `/supplies/{id}/{slug of its name}`. Any
//! other spelling (no slug, an old slug after a rename, `007` for `7`) answers
//! 301 to it, so shared links keep working and search engines index one page.
//! Like a service's page it carries the seller (call button, their page) and
//! the contact form, about this product; it posts to /contact with
//! `return_to=product`, so the visitor lands back here.

use application::{
    features::Feature,
    outputs::{BusinessDetailDto, ProductDto},
};
use axum::{extract::State, http::StatusCode, response::Response};
use domain::{Error, ErrorCode};
use serde::Serialize;

use crate::{
    app_state::AppState,
    constants::META_DESCRIPTION_MAX_CHARS,
    cookies::{Flash, TakeFlash},
    extract::ApiPath,
    middleware::auth::Viewer,
    pages::{
        canonical_url,
        manage::{manage_link, Managed},
        moved_permanently, parse_id,
        prices::commodity_href,
        provider::{
            area_of, rating_line, ContactLimits, ContactState, RatingLine, Visit, LIMITS,
            RETURN_TO_PRODUCT,
        },
        service::{service_links, OtherService},
        summary,
    },
    routes,
    shell::{Link, Shell},
    views::render_page_status,
};

#[derive(Serialize)]
pub struct ProductPage {
    /// The provider's rating and the way to its reviews.
    pub rating: RatingLine,
    pub product: ProductDto,
    /// The seller, with the services the contact form could offer.
    pub provider: BusinessDetailDto,
    /// The description shortened for `<meta name="description">`; `None`
    /// when the product has none (the template writes a sentence instead).
    pub meta_description: Option<String>,
    /// The seller's page.
    pub provider_href: String,
    /// The products of its category: where the back link leads, so a
    /// visitor who browsed one category goes back to it.
    pub category_href: String,
    /// What else the seller does: their services, each to its own page.
    pub provider_services: Vec<OtherService>,
    /// Where the seller works, or the region when the address is unknown.
    pub area: String,
    pub share_text: String,
    pub contact: ContactState,
    pub limits: ContactLimits,
    /// The form's hidden `return_to`.
    pub return_to: &'static str,
    /// The form was just sent (flash from the redirect).
    pub sent: bool,
    /// Where the owner or an admin edits it (`pages::manage`).
    pub manage: Option<Link>,
    /// The price board of what it is for, when that has a fresh price.
    pub prices_href: Option<String>,
}

async fn load(state: &AppState, raw_id: &str) -> Result<ProductDto, Error> {
    let id = parse_id(raw_id, ErrorCode::ProductNotFound)?;
    state.usecases.product.get_product_by_id(id).await
}

pub(super) fn canonical(product: &ProductDto) -> Result<String, Error> {
    canonical_url("product", product.id, &product.name)
}

/// The page around `product`, with the contact form as `contact` left it
/// (fresh, or re-shown after a refusal). `provider` is the seller.
pub(super) async fn render(
    state: &AppState,
    shell: &Shell,
    product: ProductDto,
    provider: BusinessDetailDto,
    mut contact: ContactState,
    visit: Visit,
    status: StatusCode,
) -> Result<Response, Error> {
    // A fresh form is about this product.
    if contact.values.topic.is_empty() {
        contact.values.topic = product.name.clone();
    }
    let area = area_of(state.locale(), provider.address.as_deref());
    let rating = rating_line(state, shell, &provider).await?;
    let prices_href = match product.commodity_id {
        Some(id)
            if shell.shows(Feature::Prices)
                && state.usecases.price.has_fresh_prices(id).await? =>
        {
            Some(commodity_href(id)?)
        }
        _ => None,
    };
    let page = ProductPage {
        rating,
        provider_services: service_links(&provider.services, None)?,
        provider_href: canonical_url("provider", provider.id, &provider.name)?,
        category_href: routes::url(
            "product_category",
            &[
                ("id", &product.category_id.to_string()),
                ("slug", &product.category_slug),
            ],
        )
        .map_err(|e| Error::internal(format!("product page links: {e}")))?,
        meta_description: product
            .description
            .as_deref()
            .and_then(|d| summary(d, META_DESCRIPTION_MAX_CHARS)),
        share_text: state.locale().text_with(
            "product.share_text",
            &[
                ("name", &product.name),
                ("vendor", &product.vendor_name),
                ("area", &area),
            ],
        ),
        area,
        product,
        provider,
        contact,
        limits: LIMITS,
        return_to: RETURN_TO_PRODUCT,
        sent: visit.sent,
        manage: visit.manage,
        prices_href,
    };
    render_page_status(state, shell, status, "pages/product.html", &page)
}

/// GET /supplies/{id}
pub async fn by_id(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> Result<Response, Error> {
    let product = load(&state, &id).await?;
    moved_permanently(&canonical(&product)?)
}

/// GET /supplies/{id}/{slug}
pub async fn show(
    shell: Shell,
    Viewer(viewer): Viewer,
    flash: TakeFlash,
    State(state): State<AppState>,
    ApiPath((id, _slug)): ApiPath<(String, String)>,
) -> Result<Response, Error> {
    let product = load(&state, &id).await?;
    let canonical = canonical(&product)?;
    if shell.path != canonical {
        return moved_permanently(&canonical);
    }
    let provider = state
        .usecases
        .explore
        .get_provider_detail(product.business_id)
        .await?;
    let visit = Visit {
        sent: flash.0 == Some(Flash::ContactSent),
        manage: manage_link(
            &state,
            viewer.as_ref(),
            product.business_id,
            Managed::Product(product.id),
        )
        .await?,
    };
    let mut response = render(
        &state,
        &shell,
        product,
        provider,
        ContactState::default(),
        visit,
        StatusCode::OK,
    )
    .await?;
    flash.clear_on(&mut response);
    Ok(response)
}
